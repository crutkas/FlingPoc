use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AirPlayError, Result};

const STORE_VERSION: u32 = 1;

/// Long-term HAP identity material for one paired receiver.
///
/// `controller_seed` is private key material and this type deliberately does
/// not implement `Debug`.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HapCredentials {
    pub controller_id: String,
    pub controller_seed: [u8; 32],
    pub accessory_id: String,
    pub accessory_public_key: [u8; 32],
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialFile {
    version: u32,
    devices: BTreeMap<String, HapCredentials>,
}

/// Per-user JSON credential store compatible with future HAP state machines.
pub struct FileCredentialStore {
    path: PathBuf,
}

impl FileCredentialStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Use `~/.fling/airplay-credentials.json`.
    pub fn default_for_user() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| {
                AirPlayError::CredentialStore(
                    "HOME or USERPROFILE must be set for credential storage.".to_string(),
                )
            })?;
        Ok(Self::new(
            PathBuf::from(home)
                .join(".fling")
                .join("airplay-credentials.json"),
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, device_id: &str) -> Result<Option<HapCredentials>> {
        Ok(self.load()?.devices.get(device_id).cloned())
    }

    pub fn put(&self, device_id: impl Into<String>, credentials: HapCredentials) -> Result<()> {
        let mut file = self.load()?;
        file.devices.insert(device_id.into(), credentials);
        self.save(&file)
    }

    fn load(&self) -> Result<CredentialFile> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CredentialFile {
                    version: STORE_VERSION,
                    devices: BTreeMap::new(),
                });
            }
            Err(error) => return Err(error.into()),
        };
        let file: CredentialFile = serde_json::from_slice(&bytes)?;
        if file.version != STORE_VERSION {
            return Err(AirPlayError::CredentialStore(format!(
                "Unsupported credential store version {}.",
                file.version
            )));
        }
        Ok(file)
    }

    fn save(&self, file: &CredentialFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut output = options.open(&self.path)?;
        let mut encoded = serde_json::to_vec_pretty(file)?;
        encoded.push(b'\n');
        output.write_all(&encoded)?;
        output.sync_all()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn credentials_round_trip_without_debug_exposure() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fling-airplay-credentials-{}-{suffix}.json",
            std::process::id()
        ));
        let store = FileCredentialStore::new(&path);
        let credentials = HapCredentials {
            controller_id: "controller".to_string(),
            controller_seed: [7; 32],
            accessory_id: "accessory".to_string(),
            accessory_public_key: [9; 32],
        };

        store
            .put("living-room", credentials.clone())
            .expect("credentials are saved");
        assert!(store.get("unknown").expect("store can be read").is_none());
        let stored = store
            .get("living-room")
            .expect("store can be read")
            .expect("credential exists");
        assert!(stored == credentials);

        fs::remove_file(path).expect("temporary credential file is removed");
    }
}
