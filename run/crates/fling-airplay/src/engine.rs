use crate::{
    Capabilities, Device, MdnsDiscovery, PairingSession, PlaybackStatus, Result,
    error::AirPlayError,
};

const PAIRING_UNAVAILABLE: &str =
    "Rust HAP pairing is not available yet; select the Python bridge for pairing.";
const URL_PLAYBACK_UNAVAILABLE: &str =
    "Rust AirPlay URL playback is not available yet; select the Python bridge.";
const FILE_PLAYBACK_UNAVAILABLE: &str =
    "Rust LAN file playback is not available yet; select the Python bridge.";
const HLS_MIRRORING_UNAVAILABLE: &str = "Rust HLS mirroring is not available yet; select the Python bridge. Native AirPlay mirroring remains unsupported until a separately licensed FairPlay implementation is available.";
const STATUS_UNAVAILABLE: &str =
    "Rust playback status is not available yet; select the Python bridge.";
const STOP_UNAVAILABLE: &str =
    "Rust playback control is not available yet; select the Python bridge.";

/// Reusable `AirPlay` engine behind the language-neutral sidecar contract.
pub struct AirPlayEngine {
    discovery: MdnsDiscovery,
}

impl AirPlayEngine {
    #[must_use]
    pub fn new(discovery: MdnsDiscovery) -> Self {
        Self { discovery }
    }

    #[must_use]
    pub fn capabilities(&self) -> Capabilities {
        Capabilities::rust_foundation()
    }

    /// Discover receivers.
    ///
    /// # Errors
    ///
    /// Returns an error if mDNS discovery fails.
    pub async fn devices(&self) -> Result<Vec<Device>> {
        self.discovery.devices().await
    }

    /// Start pairing after the HAP implementation is enabled.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn start_pairing(&self, _device_id: &str) -> Result<PairingSession> {
        Err(AirPlayError::Unsupported(PAIRING_UNAVAILABLE))
    }

    /// Finish pairing after the HAP implementation is enabled.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn finish_pairing(&self, _session_id: &str, _pin: &str) -> Result<()> {
        Err(AirPlayError::Unsupported(PAIRING_UNAVAILABLE))
    }

    /// Start URL playback after pair verification is enabled.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn play_url(&self, _device_id: &str, _url: &str) -> Result<()> {
        Err(AirPlayError::Unsupported(URL_PLAYBACK_UNAVAILABLE))
    }

    /// Start LAN file playback after media serving is enabled.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn play_file(&self, _device_id: &str, _path: &str) -> Result<()> {
        Err(AirPlayError::Unsupported(FILE_PLAYBACK_UNAVAILABLE))
    }

    /// Start the distributable HLS mirroring fallback after it is ported.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn mirror_hls(&self, _device_id: &str) -> Result<()> {
        Err(AirPlayError::Unsupported(HLS_MIRRORING_UNAVAILABLE))
    }

    /// Read playback status after session control is enabled.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn status(&self, _device_id: &str) -> Result<PlaybackStatus> {
        Err(AirPlayError::Unsupported(STATUS_UNAVAILABLE))
    }

    /// Stop playback after session control is enabled.
    ///
    /// # Errors
    ///
    /// This increment always returns an unsupported error.
    pub fn stop(&self, _device_id: &str) -> Result<()> {
        Err(AirPlayError::Unsupported(STOP_UNAVAILABLE))
    }
}

impl Default for AirPlayEngine {
    fn default() -> Self {
        Self::new(MdnsDiscovery::default())
    }
}
