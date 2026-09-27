use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineGroup {
    pub name: String,
    #[serde(default)]
    pub host_ids: Vec<String>,
}

impl MachineGroup {
    pub fn empty(name: String) -> Self {
        Self {
            name,
            host_ids: Vec::new(),
        }
    }
}

pub fn state_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("nekohub")
        .join("machine-groups.json")
}

pub fn load(path: &Path) -> Vec<MachineGroup> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub async fn save(path: &Path, groups: &[MachineGroup]) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(groups)
        .map_err(|error| format!("Could not encode machine groups: {error}"))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Could not create the settings folder: {error}"))?;
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| format!("Could not save machine groups: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn groups_round_trip_through_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("groups.json");
        let expected = vec![MachineGroup {
            name: "Production".into(),
            host_ids: vec!["api-1".into()],
        }];

        save(&path, &expected).await.unwrap();

        assert_eq!(load(&path), expected);
    }
}
