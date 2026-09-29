use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineCredential {
    pub password: Option<String>,
    pub identity_file: Option<PathBuf>,
}

impl fmt::Debug for MachineCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MachineCredential")
            .field("password", &self.password.as_ref().map(|_| "[redacted]"))
            .field("identity_file", &self.identity_file)
            .finish()
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialStore {
    #[serde(default)]
    machines: BTreeMap<String, MachineCredential>,
}

impl fmt::Debug for CredentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialStore")
            .field("machine_count", &self.machines.len())
            .finish()
    }
}

impl CredentialStore {
    pub fn get(&self, alias: &str) -> Option<&MachineCredential> {
        self.machines.get(alias)
    }

    pub fn aliases(&self) -> impl Iterator<Item = &str> {
        self.machines.keys().map(String::as_str)
    }

    pub fn save_password(&mut self, alias: &str, password: String) {
        self.machines.entry(alias.to_owned()).or_default().password = Some(password);
    }

    pub fn remove_password(&mut self, alias: &str) -> bool {
        let removed = self
            .machines
            .get_mut(alias)
            .and_then(|credential| credential.password.take())
            .is_some();
        self.remove_empty(alias);
        removed
    }

    pub fn save_identity_file(&mut self, alias: &str, identity_file: PathBuf) {
        self.machines
            .entry(alias.to_owned())
            .or_default()
            .identity_file = Some(identity_file);
    }

    pub fn remove_identity_file(&mut self, alias: &str) -> bool {
        let removed = self
            .machines
            .get_mut(alias)
            .and_then(|credential| credential.identity_file.take())
            .is_some();
        self.remove_empty(alias);
        removed
    }

    fn remove_empty(&mut self, alias: &str) {
        if self.machines.get(alias).is_some_and(|credential| {
            credential.password.is_none() && credential.identity_file.is_none()
        }) {
            self.machines.remove(alias);
        }
    }
}

pub fn state_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("nekohub")
        .join("credentials.json")
}

pub fn load(path: &Path) -> CredentialStore {
    let Ok(contents) = fs::read_to_string(path) else {
        return CredentialStore::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save(path: &Path, credentials: &CredentialStore) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(credentials)
        .map_err(|error| format!("Could not encode credentials: {error}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the credentials folder: {error}"))?;
    }
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not open the credentials file: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not protect the credentials file: {error}"))?;
    }
    file.write_all(&contents)
        .map_err(|error| format!("Could not save credentials: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn credentials_round_trip_without_debugging_passwords() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.json");
        let mut expected = CredentialStore::default();
        expected.save_password("server", "secret".into());
        expected.save_identity_file("server", PathBuf::from("/tmp/id_server"));

        save(&path, &expected).unwrap();
        let loaded = load(&path);

        assert_eq!(loaded, expected);
        assert!(!format!("{loaded:?}").contains("secret"));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
