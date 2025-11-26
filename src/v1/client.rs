use super::model::Directory;
use crate::common::RelativePath;
use std::error::Error;

pub trait WorkspaceApi<E: Error + Send + Sync> {
    fn fetch_directory(&self, path: &RelativePath) -> impl Future<Output = Result<Option<Directory>, E>> + Send;
}
