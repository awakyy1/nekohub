use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
};

use glob::glob;
use thiserror::Error;

use crate::HostTarget;

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub hosts: Vec<HostTarget>,
}

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("cannot read SSH config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid Include pattern in {path}: {pattern}")]
    Include { path: PathBuf, pattern: String },
}

impl Inventory {
    pub fn from_ssh_config(path: &Path) -> Result<Self, InventoryError> {
        let mut aliases = BTreeSet::new();
        let mut visited = HashSet::new();
        parse_file(path, &mut aliases, &mut visited)?;
        Ok(Self {
            hosts: aliases.into_iter().map(HostTarget::from_alias).collect(),
        })
    }

    pub fn retain_aliases(&mut self, aliases: &[String]) {
        if aliases.is_empty() {
            return;
        }
        let wanted: HashSet<&str> = aliases.iter().map(String::as_str).collect();
        self.hosts
            .retain(|host| wanted.contains(host.alias.as_str()));
    }
}

fn parse_file(
    path: &Path,
    aliases: &mut BTreeSet<String>,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), InventoryError> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(());
    }
    let content = fs::read_to_string(path).map_err(|source| InventoryError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        let Some((keyword, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if keyword.eq_ignore_ascii_case("host") {
            for alias in value
                .split_whitespace()
                .filter(|value| is_concrete_alias(value))
            {
                aliases.insert(alias.to_owned());
            }
        } else if keyword.eq_ignore_ascii_case("include") {
            for pattern in value.split_whitespace() {
                let expanded = expand_include(pattern, parent);
                let rendered = expanded.to_string_lossy().into_owned();
                let paths = glob(&rendered).map_err(|_| InventoryError::Include {
                    path: path.to_path_buf(),
                    pattern: rendered.clone(),
                })?;
                for included in paths.flatten().filter(|candidate| candidate.is_file()) {
                    parse_file(&included, aliases, visited)?;
                }
            }
        }
    }
    Ok(())
}

fn is_concrete_alias(value: &&str) -> bool {
    !value.starts_with('!') && !value.chars().any(|ch| matches!(ch, '*' | '?' | '[' | ']'))
}

fn expand_include(value: &str, parent: &Path) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    {
        return PathBuf::from(home).join(rest);
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        parent.join(path)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn discovers_only_concrete_aliases() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "Host prod-a prod-b\n  User ops\nHost *.internal !blocked\n  User ignored"
        )
        .unwrap();
        let inventory = Inventory::from_ssh_config(file.path()).unwrap();
        let aliases: Vec<_> = inventory
            .hosts
            .iter()
            .map(|host| host.alias.as_str())
            .collect();
        assert_eq!(aliases, ["prod-a", "prod-b"]);
    }
}
