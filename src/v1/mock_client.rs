// == Std
use std::{ops::Range, time::Duration};
// == Internal crates
use super::{
    client::WorkspaceApi,
    model::{Directory, DirectoryEntryType},
};
use crate::common::RelativePath;
// == External crates
use thiserror::Error;
use tokio::time::sleep;

#[derive(Debug, Clone, Error)]
pub enum MockWorkspaceError {}

pub struct MockWorkspaceApi {
    full_directory_tree: Directory,
    /// Simulated latency range for requests, in milliseconds, each request will be delayed by a random number of
    /// milliseconds within this range
    request_latency_range_ms: Range<u32>,
}

impl MockWorkspaceApi {
    async fn delay(&self) {
        let delay_ms = rand::random_range(self.request_latency_range_ms.clone());
        if delay_ms > 0 {
            eprintln!("MockWorkspaceApi delaying request by {} ms", delay_ms);
        }
        sleep(Duration::from_millis(delay_ms as u64)).await;
    }
}

impl WorkspaceApi<MockWorkspaceError> for MockWorkspaceApi {
    async fn fetch_directory(&self, path: &RelativePath) -> Result<Option<Directory>, MockWorkspaceError> {
        self.delay().await;

        if path.is_empty() {
            Ok(Some(self.full_directory_tree.clone()))
        } else {
            let mut current = &self.full_directory_tree;

            for component in path.components() {
                // Find the component in the current directory - inefficient but acceptable for a mock
                let entry_opt = current.entries().iter().find(|entry| entry.name() == component);
                if let Some(entry) = entry_opt {
                    match entry.info() {
                        DirectoryEntryType::Directory(Some(dir_info)) => {
                            current = dir_info;
                        }
                        DirectoryEntryType::Directory(None) => {
                            // The mock directory tree will not contain any unloaded directories, so this is an error
                            panic!("Mock directory tree should not contain unloaded directories");
                        }
                        DirectoryEntryType::File { .. } => {
                            // Entry is a file, so the path cannot continue
                            return Ok(None);
                        }
                    }
                } else {
                    // Component not found
                    return Ok(None);
                }
            }

            Ok(Some(current.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::model::*;

    #[tokio::test]
    async fn test_fetch_directory() {
        let mut root = Directory::new(RelativePath::new("").unwrap(), vec![]);

        let mut sub_dir = Directory::new(RelativePath::new("subdir").unwrap(), vec![]);

        let mut sub_sub_dir = Directory::new(RelativePath::new("subdir/nested").unwrap(), vec![]);

        sub_sub_dir.push_entry(DirectoryEntry::new(
            "file.txt".into(),
            DirectoryEntryType::File {
                metadata: FileMetadata::new(0, 0),
                change_state: Default::default(),
                conflict_state: Default::default(),
            },
        ));

        sub_dir.push_entry(DirectoryEntry::new(
            "nested".into(),
            DirectoryEntryType::Directory(Some(sub_sub_dir)),
        ));

        root.push_entry(DirectoryEntry::new(
            "subdir".into(),
            DirectoryEntryType::Directory(Some(sub_dir)),
        ));

        //println!("Constructed mock directory tree: {}", serde_json::to_string_pretty(&root).unwrap());

        let mock_api = MockWorkspaceApi {
            full_directory_tree: root,
            request_latency_range_ms: 0..1,
        };

        let dir = mock_api
            .fetch_directory(&RelativePath::new("missing/path").unwrap())
            .await
            .unwrap();
        assert!(dir.is_none());

        let dir = mock_api
            .fetch_directory(&RelativePath::new("subdir").unwrap())
            .await
            .unwrap()
            .expect("subdir should exist");
        assert_eq!(dir.relative_path().to_string(), "subdir");

        let dir = mock_api
            .fetch_directory(&RelativePath::new("subdir/nested").unwrap())
            .await
            .unwrap()
            .expect("subdir/nested should exist");
        assert_eq!(dir.relative_path().to_string(), "subdir/nested");

        let dir = mock_api
            .fetch_directory(&RelativePath::new("subdir/nested/file.txt").unwrap())
            .await
            .unwrap();
        assert!(dir.is_none());
    }
}
