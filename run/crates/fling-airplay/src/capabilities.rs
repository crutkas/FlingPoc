use serde::{Deserialize, Serialize};

/// Availability of one engine feature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityStatus {
    Supported,
    PythonFallback,
    Unsupported,
}

/// Machine-readable feature availability and an operator-facing explanation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub status: CapabilityStatus,
    pub detail: String,
}

impl Capability {
    fn new(status: CapabilityStatus, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    fn supported(detail: impl Into<String>) -> Self {
        Self::new(CapabilityStatus::Supported, detail)
    }

    fn python_fallback(detail: impl Into<String>) -> Self {
        Self::new(CapabilityStatus::PythonFallback, detail)
    }

    fn unsupported(detail: impl Into<String>) -> Self {
        Self::new(CapabilityStatus::Unsupported, detail)
    }
}

/// Capabilities exposed by the current Rust foundation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub discovery: Capability,
    pub pairing: Capability,
    pub url_playback: Capability,
    pub file_playback: Capability,
    pub playback_status: Capability,
    pub stop: Capability,
    pub hls_mirroring: Capability,
    pub native_mirroring: Capability,
}

impl Capabilities {
    /// Return the deliberately conservative capabilities of this increment.
    #[must_use]
    pub fn rust_foundation() -> Self {
        Self {
            discovery: Capability::supported(
                "AirPlay services are discovered with cross-platform mDNS.",
            ),
            pairing: Capability::python_fallback(
                "HAP pairing remains on the Python bridge while the Rust state machine is validated.",
            ),
            url_playback: Capability::python_fallback(
                "AirPlay URL playback remains on the Python bridge until pair verification is complete.",
            ),
            file_playback: Capability::python_fallback(
                "Token-gated LAN file serving remains on the Python bridge.",
            ),
            playback_status: Capability::python_fallback(
                "Playback metadata remains on the Python bridge.",
            ),
            stop: Capability::python_fallback("Playback control remains on the Python bridge."),
            hls_mirroring: Capability::python_fallback(
                "The distributable FFmpeg-to-HLS fallback remains on the Python bridge.",
            ),
            native_mirroring: Capability::unsupported(
                "Native mirroring requires a separately licensed FairPlay implementation.",
            ),
        }
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::rust_foundation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mirroring_is_explicitly_unsupported() {
        let capabilities = Capabilities::rust_foundation();

        assert_eq!(
            capabilities.native_mirroring.status,
            CapabilityStatus::Unsupported
        );
        assert!(capabilities.native_mirroring.detail.contains("FairPlay"));
        assert_eq!(
            capabilities.hls_mirroring.status,
            CapabilityStatus::PythonFallback
        );
    }
}
