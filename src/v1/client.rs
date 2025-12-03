// == Std
use std::error::Error;

// == Internal crates
use super::model::Directory;
use crate::common::RelativePath;

#[derive(Debug, Clone, Default)]
pub struct DirectoryFetchOptions {
    /// Specifies depth to fetch from the current directory, `None` means unlimited depth
    /// For example, a depth limit of 0 will only load the specified directory with no sub-directories
    pub depth_limit: Option<u32>,
    /// Optional filter string to filter directory entries by name (case-insensitive substring match)
    /// NOTE: Currently not implemented in MockWorkspaceApi
    pub filter_string: Option<String>,
}

pub trait WorkspaceApi {
    fn fetch_directory(
        &self,
        path: &RelativePath,
        options: DirectoryFetchOptions,
    ) -> impl Future<Output = Result<Option<Directory>, Box<dyn Error>>>;
}
