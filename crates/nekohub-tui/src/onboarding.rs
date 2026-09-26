use std::path::{Path, PathBuf};

const ONBOARDING_STATE: &str = "agent-setup-v1\n";

pub fn state_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("nekohub")
        .join("onboarding-state")
}

pub fn is_complete(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|state| state == ONBOARDING_STATE)
}

pub async fn mark_complete(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, ONBOARDING_STATE).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completion_is_versioned_and_persistent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state");
        assert!(!is_complete(&path));
        mark_complete(&path).await.unwrap();
        assert!(is_complete(&path));
    }
}
