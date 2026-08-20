use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use tokio::time::Instant;

use crate::{AirPlayError, Device, Result};

pub const AIRPLAY_SERVICE_TYPE: &str = "_airplay._tcp.local.";

/// Resolved AirPlay service details used by future pairing and playback code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AirPlayService {
    pub device: Device,
    pub port: u16,
    pub properties: BTreeMap<String, String>,
}

/// Cross-platform AirPlay discovery backed by DNS-SD.
pub struct MdnsDiscovery {
    timeout: Duration,
}

impl MdnsDiscovery {
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub async fn devices(&self) -> Result<Vec<Device>> {
        Ok(self
            .discover_services()
            .await?
            .into_iter()
            .map(|service| service.device)
            .collect())
    }

    pub async fn discover_services(&self) -> Result<Vec<AirPlayService>> {
        let daemon =
            ServiceDaemon::new().map_err(|error| AirPlayError::Discovery(error.to_string()))?;
        let receiver = match daemon.browse(AIRPLAY_SERVICE_TYPE) {
            Ok(receiver) => receiver,
            Err(error) => {
                let _ = daemon.shutdown();
                return Err(AirPlayError::Discovery(error.to_string()));
            }
        };

        let deadline = Instant::now() + self.timeout;
        let mut services = BTreeMap::new();
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let event = tokio::time::timeout(deadline - now, receiver.recv_async()).await;
            match event {
                Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                    if let Some(service) = service_from_resolved(&info) {
                        services.insert(service.device.id.clone(), service);
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => break,
            }
        }

        let _ = daemon.stop_browse(AIRPLAY_SERVICE_TYPE);
        let _ = daemon.shutdown();
        Ok(services.into_values().collect())
    }
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}

fn service_from_resolved(info: &ResolvedService) -> Option<AirPlayService> {
    let address = preferred_address(
        info.get_addresses()
            .iter()
            .map(mdns_sd::ScopedIp::to_ip_addr)
            .collect(),
    )?;
    let properties: BTreeMap<_, _> = info
        .get_properties()
        .iter()
        .map(|property| {
            (
                property.key().to_ascii_lowercase(),
                property.val_str().to_string(),
            )
        })
        .collect();
    let name = info
        .get_fullname()
        .strip_suffix(AIRPLAY_SERVICE_TYPE)
        .unwrap_or(info.get_fullname())
        .trim_end_matches('.')
        .to_string();
    let id = ["deviceid", "pi", "psi"]
        .iter()
        .find_map(|key| properties.get(*key))
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| name.clone());

    Some(AirPlayService {
        device: Device {
            id,
            name,
            address: address.to_string(),
            protocols: vec!["AirPlay".to_string()],
        },
        port: info.get_port(),
        properties,
    })
}

fn preferred_address(addresses: HashSet<IpAddr>) -> Option<IpAddr> {
    let mut addresses: Vec<_> = addresses.into_iter().collect();
    addresses.sort_by_key(ToString::to_string);
    addresses
        .iter()
        .copied()
        .find(|address| {
            matches!(
                address,
                IpAddr::V4(ipv4) if !ipv4.is_link_local() && !ipv4.is_loopback()
            )
        })
        .or_else(|| {
            addresses
                .iter()
                .copied()
                .find(|address| !address.is_loopback())
        })
        .or_else(|| addresses.first().copied())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn address_selection_prefers_routable_ipv4() {
        let addresses = HashSet::from([
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            "2001:db8::1".parse().expect("valid IPv6 address"),
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)),
        ]);

        assert_eq!(
            preferred_address(addresses),
            Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)))
        );
    }
}
