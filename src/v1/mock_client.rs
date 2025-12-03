// == Std
use std::{ops::Range, path::Path, time::Duration};
// == Internal crates
use super::{
    client::{DirectoryFetchOptions, WorkspaceApi},
    model::{Directory, DirectoryEntryType},
};
use crate::common::RelativePath;
// == External crates
use thiserror::Error;
use tokio::time::sleep;

pub struct MockWorkspaceApi {
    full_directory_tree: Directory,
    /// Simulated latency range for requests, in milliseconds, each request will be delayed by a random number of
    /// milliseconds within this range
    request_latency_range_ms: Range<u32>,
}

#[derive(Debug, Error)]
pub enum MockWorkspaceApiJsonError {
    #[error("Failed to parse JSON data: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

impl Default for MockWorkspaceApi {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWorkspaceApi {
    pub fn new() -> Self {
        MockWorkspaceApi {
            full_directory_tree: Directory::new(RelativePath::new("").unwrap(), vec![]),
            request_latency_range_ms: 0..1,
        }
    }

    pub async fn set_directory_tree_from_json_str(&mut self, json_data: &str) -> Result<(), MockWorkspaceApiJsonError> {
        let directory: Directory = serde_json::from_str(json_data)?;
        self.full_directory_tree = directory;

        Ok(())
    }

    pub async fn set_directory_tree_from_json_file(
        &mut self,
        json_file_path: &Path,
    ) -> Result<(), MockWorkspaceApiJsonError> {
        let json = tokio::fs::read_to_string(json_file_path).await?;
        self.set_directory_tree_from_json_str(&json).await
    }

    async fn delay(&self) {
        let delay_ms = rand::random_range(self.request_latency_range_ms.clone());
        if delay_ms > 0 {
            eprintln!("MockWorkspaceApi delaying request by {} ms", delay_ms);
        }
        sleep(Duration::from_millis(delay_ms as u64)).await;
    }
}

impl WorkspaceApi for MockWorkspaceApi {
    async fn fetch_directory(
        &self,
        path: &RelativePath,
        options: DirectoryFetchOptions,
    ) -> Result<Option<Directory>, Box<dyn std::error::Error>> {
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

            let mut directory = current.clone();
            if let Some(depth_limit) = options.depth_limit {
                // Cull entries beyond the depth limit
                directory.prune_to_depth(depth_limit);
            }

            Ok(Some(directory))
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

        let fetch_options = DirectoryFetchOptions::default();

        let dir = mock_api
            .fetch_directory(&RelativePath::new("missing/path").unwrap(), fetch_options.clone())
            .await
            .unwrap();
        assert!(dir.is_none());

        let dir = mock_api
            .fetch_directory(&RelativePath::new("subdir").unwrap(), fetch_options.clone())
            .await
            .unwrap()
            .expect("subdir should exist");
        assert_eq!(dir.relative_path().to_string(), "subdir");

        let dir = mock_api
            .fetch_directory(&RelativePath::new("subdir/nested").unwrap(), fetch_options.clone())
            .await
            .unwrap()
            .expect("subdir/nested should exist");
        assert_eq!(dir.relative_path().to_string(), "subdir/nested");

        let dir = mock_api
            .fetch_directory(
                &RelativePath::new("subdir/nested/file.txt").unwrap(),
                fetch_options.clone(),
            )
            .await
            .unwrap();
        assert!(dir.is_none());
    }

    #[tokio::test]
    async fn test_json_data() {
        let test_json_data = include_str!("test_data/lyra.json");
        let mut mock_api = MockWorkspaceApi::default();

        let result = mock_api
            .fetch_directory(&RelativePath::new("").unwrap(), DirectoryFetchOptions::default())
            .await
            .unwrap();
        assert!(
            result.unwrap().entries().is_empty(),
            "Initial mock directory should be empty"
        );

        mock_api
            .set_directory_tree_from_json_str(test_json_data)
            .await
            .expect("Setting directory tree from JSON should succeed");

        let result = mock_api
            .fetch_directory(&RelativePath::new("").unwrap(), DirectoryFetchOptions::default())
            .await
            .unwrap();
        assert!(
            !result.unwrap().entries().is_empty(),
            "Mock directory should not be empty after setting JSON data"
        );
    }
}
