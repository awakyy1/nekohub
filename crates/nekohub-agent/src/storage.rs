use std::{collections::HashSet, fs, os::unix::fs::MetadataExt, path::Path, time::Instant};

use nekohub_core::{StorageEntry, StorageSnapshot};

const MAX_VISITED_ENTRIES: u64 = 500_000;

#[derive(Default)]
struct ScanState {
    visited: u64,
    files: u64,
    unreadable: u64,
    truncated: bool,
    hard_links: HashSet<(u64, u64)>,
}

pub(crate) fn scan_path(root: &Path, total_bytes: u64, used_bytes: u64) -> StorageSnapshot {
    let started = Instant::now();
    let root_device = fs::symlink_metadata(root).map_or(0, |metadata| metadata.dev());
    let mut state = ScanState::default();
    let mut entries = Vec::new();

    match fs::read_dir(root) {
        Ok(children) => {
            for child in children {
                if state.truncated {
                    break;
                }
                let Ok(child) = child else {
                    state.unreadable += 1;
                    continue;
                };
                let path = child.path();
                let Ok(metadata) = fs::symlink_metadata(&path) else {
                    state.unreadable += 1;
                    continue;
                };
                if metadata.file_type().is_symlink() || metadata.dev() != root_device {
                    continue;
                }
                let is_directory = metadata.is_dir();
                let files_before = state.files;
                let allocated_bytes = measure(&path, &metadata, root_device, &mut state);
                entries.push(StorageEntry {
                    name: child.file_name().to_string_lossy().into_owned(),
                    path: path.to_string_lossy().into_owned(),
                    allocated_bytes,
                    file_count: state.files.saturating_sub(files_before),
                    is_directory,
                });
            }
        }
        Err(_) => state.unreadable += 1,
    }
    entries.sort_by(|left, right| right.allocated_bytes.cmp(&left.allocated_bytes));
    let scanned_bytes = entries.iter().map(|entry| entry.allocated_bytes).sum();

    StorageSnapshot {
        root: root.to_string_lossy().into_owned(),
        total_bytes,
        used_bytes,
        scanned_bytes,
        file_count: state.files,
        unreadable_entries: state.unreadable,
        truncated: state.truncated,
        elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        entries,
    }
}

fn measure(path: &Path, metadata: &fs::Metadata, root_device: u64, state: &mut ScanState) -> u64 {
    if state.visited >= MAX_VISITED_ENTRIES {
        state.truncated = true;
        return 0;
    }
    state.visited += 1;

    if metadata.is_file() {
        state.files += 1;
        if metadata.nlink() > 1 && !state.hard_links.insert((metadata.dev(), metadata.ino())) {
            return 0;
        }
        return metadata.blocks().saturating_mul(512);
    }
    if !metadata.is_dir() {
        return 0;
    }

    let mut bytes = metadata.blocks().saturating_mul(512);
    let children = match fs::read_dir(path) {
        Ok(children) => children,
        Err(_) => {
            state.unreadable += 1;
            return bytes;
        }
    };
    for child in children {
        if state.truncated {
            break;
        }
        let Ok(child) = child else {
            state.unreadable += 1;
            continue;
        };
        let child_path = child.path();
        let Ok(child_metadata) = fs::symlink_metadata(&child_path) else {
            state.unreadable += 1;
            continue;
        };
        if child_metadata.file_type().is_symlink() || child_metadata.dev() != root_device {
            continue;
        }
        bytes = bytes.saturating_add(measure(&child_path, &child_metadata, root_device, state));
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_children_without_following_symlinks() {
        let temp = tempfile::tempdir().expect("temporary directory");
        fs::create_dir(temp.path().join("home")).expect("directory");
        fs::write(temp.path().join("home/data.bin"), vec![7_u8; 8192]).expect("file");
        std::os::unix::fs::symlink(temp.path(), temp.path().join("home/loop")).expect("symlink");

        let snapshot = scan_path(temp.path(), 100_000, 10_000);

        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].name, "home");
        assert_eq!(snapshot.file_count, 1);
        assert!(snapshot.scanned_bytes > 0);
        assert!(!snapshot.truncated);
    }
}
