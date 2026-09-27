use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default = "background_enabled_by_default")]
    pub background_enabled: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            background_enabled: background_enabled_by_default(),
        }
    }
}

pub fn state_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("nekohub")
        .join("preferences.json")
}

pub fn load(path: &Path) -> Preferences {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Preferences::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub async fn save(path: &Path, preferences: Preferences) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(&preferences)
        .map_err(|error| format!("Could not encode preferences: {error}"))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Could not create the settings folder: {error}"))?;
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| format!("Could not save preferences: {error}"))
}

const fn background_enabled_by_default() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preferences_round_trip_through_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let expected = Preferences {
            background_enabled: false,
        };

        save(&path, expected).await.unwrap();

        assert!(!load(&path).background_enabled);
    }
}
