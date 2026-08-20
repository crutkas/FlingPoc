use serde::{Deserialize, Serialize};

/// Device response in the existing `/v1/devices` contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub address: String,
    pub protocols: Vec<String>,
}

/// Pairing response in the existing `/v1/pair/start` contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingSession {
    pub session_id: String,
}

/// Playback response in the existing `/v1/status` contract.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatus {
    pub state: String,
    pub title: Option<String>,
    pub position: Option<f64>,
    pub duration: Option<f64>,
}
