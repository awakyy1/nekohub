use std::path::{Path, PathBuf};

use nekohub_core::HostTarget;

pub const INSTALLED_TAG: &str = "agent-installed";

pub fn state_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("nekohub")
        .join("registered-machines.json")
}

pub fn load(path: &Path) -> Vec<HostTarget> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn merge(discovered: &mut Vec<HostTarget>, registered: Vec<HostTarget>) {
    for machine in registered {
        if let Some(existing) = discovered
            .iter_mut()
            .find(|existing| existing.alias == machine.alias)
        {
            existing.display_name = machine.display_name;
            for tag in machine.tags {
                if !existing.tags.contains(&tag) {
                    existing.tags.push(tag);
                }
            }
        } else {
            discovered.push(machine);
        }
    }
}

pub async fn save(path: &Path, machines: &[HostTarget]) -> Result<(), String> {
    let registered = machines
        .iter()
        .filter(|machine| machine.tags.iter().any(|tag| tag == INSTALLED_TAG))
        .collect::<Vec<_>>();
    let contents = serde_json::to_vec_pretty(&registered)
        .map_err(|error| format!("Could not encode registered machines: {error}"))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Could not create the machine registry: {error}"))?;
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| format!("Could not save registered machines: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_agent_metadata_wins_over_discovery() {
        let mut discovered = vec![HostTarget::from_alias("ops@server")];
        let mut registered = HostTarget::from_alias("ops@server");
        registered.tags.push(INSTALLED_TAG.into());
        merge(&mut discovered, vec![registered]);
        assert!(discovered[0].tags.iter().any(|tag| tag == INSTALLED_TAG));
    }
}
