//! Reusable, provenance-safe AirPlay building blocks for Fling.
//!
//! The first increment provides mDNS discovery, credential persistence, the
//! `/v1` data model, and plain RTSP/binary-plist transport. Pairing and playback
//! are intentionally capability-gated until their protocol state machines are
//! complete and hardware-validated. Native mirroring and FairPlay are out of
//! scope.

mod capabilities;
mod credentials;
mod discovery;
mod error;
mod model;
mod rtsp;

pub use capabilities::{Capabilities, Capability, CapabilityStatus};
pub use credentials::{FileCredentialStore, HapCredentials};
pub use discovery::{AIRPLAY_SERVICE_TYPE, AirPlayService, MdnsDiscovery};
pub use error::{AirPlayError, Result};
pub use model::{Device, PairingSession, PlaybackStatus};
pub use rtsp::{RtspConnection, RtspRequest, RtspResponse, decode_plist, encode_binary_plist};
