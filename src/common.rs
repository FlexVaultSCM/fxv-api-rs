// == Std
use std::{fmt::Display, iter::FusedIterator};

// == External crates
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with RelativePath
#[derive(Debug, Clone, Error)]
pub enum RelativePathError {
    #[error("The provided path '{0}' is invalid as a relative path")]
    InvalidPath(String),
}

/// Newtype for a path relative to some arbitrary root.
/// This is used to represent paths within a workspace, it contains a subset of the functionality of std::path::Path,
/// but without the platform-specific behavior. It does not support relative components like `..`, nor absolute paths,
/// and always uses `/` as the separator. It is always normalized, and always transformable to UTF-8.  Non-UTF-8 paths
/// are not supported for now.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RelativePath(String);

impl Display for RelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl RelativePath {
    /// Creates a new RelativePath from the given string.  Will normalize separators to `/`.
    pub fn new(path: impl AsRef<str>) -> Result<Self, RelativePathError> {
        let path_string = Self::normalize_separators(path.as_ref());
        if path_string.starts_with('/') || path_string.ends_with('/') {
            return Err(RelativePathError::InvalidPath(path_string));
        }

        Ok(RelativePath(path_string))
    }

    /// Returns the file name of the path, if any
    pub fn file_name(&self) -> Option<&str> {
        if self.0.is_empty() {
            None
        } else {
            // Invariants forbid a string ending or starting with a separator, so this is safe
            let index = self.0.rfind('/').map(|i| i + 1).unwrap_or(0);
            Some(&self.0[index..])
        }
    }

    /// Returns an iterator over the components of the relative path
    pub fn components<'a>(&'a self) -> RelativePathComponents<'a> {
        RelativePathComponents { inner: self, index: 0 }
    }

    /// Returns true if the relative path is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn normalize_separators(path: &str) -> String {
        path.replace("\\", "/")
    }
}

impl Ord for RelativePath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl PartialOrd for RelativePath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// An iterator over the components of a RelativePath
pub struct RelativePathComponents<'a> {
    inner: &'a RelativePath,
    index: usize,
}

impl<'a> Iterator for RelativePathComponents<'a> {
    type Item = &'a str;

    /// Returns the next component of the relative path, or None if there are no more components
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.inner.0.len() {
            None
        } else {
            let next_index = self.inner.0[self.index..]
                .find('/')
                .unwrap_or(self.inner.0.len() - self.index);
            let component = &self.inner.0[self.index..self.index + next_index];
            self.index += next_index + 1; // +1 to skip the separator
            Some(component)
        }
    }
}

impl<'a> FusedIterator for RelativePathComponents<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relative_path_creation() {
        let path = RelativePath::new("some/path/to/file.txt").unwrap();
        let path_with_backslashes = RelativePath::new("some\\path\\to\\file.txt").unwrap();

        assert_eq!(
            path.to_string(),
            "some/path/to/file.txt",
            "Path string should match expected format"
        );
        assert_eq!(
            path, path_with_backslashes,
            "Paths with different separators should be equal"
        );

        let path = RelativePath::default();
        assert_eq!(path.to_string(), "", "Default RelativePath should be empty string");

        // Test invalid paths
        let invalid_path = RelativePath::new("/absolute/path");
        assert!(invalid_path.is_err(), "Absolute paths should be invalid");

        let invalid_path = RelativePath::new("trailing/slash/");
        assert!(invalid_path.is_err(), "Paths with trailing slashes should be invalid");

        let invalid_path = RelativePath::new("/");
        assert!(invalid_path.is_err(), "Single slash path should be invalid");

        // These should also fail, but the current implementation doesn't check for these cases, uncomment when
        // implemented
        /*
        let invalid_path = RelativePath::new("some/../path");
        assert!(invalid_path.is_err(), "Relative components should be invalid");

        let invalid_path = RelativePath::new("some/./path");
        //assert!(invalid_path.is_err(), "Current directory components should be invalid");

        let invalid_path = RelativePath::new("some//path");
        //assert!(invalid_path.is_err(), "Consecutive separators should be invalid");
        */
    }

    #[test]
    fn test_relative_path_file_name() {
        let path = RelativePath::new("some/path/to/file.txt").unwrap();
        assert_eq!(path.file_name(), Some("file.txt"), "File name should be 'file.txt'");

        let root_path = RelativePath::new("").unwrap();
        assert_eq!(root_path.file_name(), None, "File name of empty path should be None");
    }

    #[test]
    fn test_relative_path_components() {
        let path = RelativePath::new("some/path/to/file.txt").unwrap();

        // Test fused iterator behavior
        let mut components = path.components();
        assert_eq!(components.next(), Some("some"), "First component should be 'some'");
        assert_eq!(components.next(), Some("path"), "Second component should be 'path'");
        assert_eq!(components.next(), Some("to"), "Third component should be 'to'");
        assert_eq!(
            components.next(),
            Some("file.txt"),
            "Fourth component should be 'file.txt'"
        );
        assert_eq!(components.next(), None, "No more components should be present");
        assert_eq!(
            components.next(),
            None,
            "No more components should be present (fused iterator)"
        );

        // Test using iterator
        let components_vec = path.components().collect::<Vec<_>>();
        assert_eq!(
            components_vec,
            vec!["some", "path", "to", "file.txt"],
            "Components should match expected values"
        );

        let path = RelativePath::new("").unwrap();
        assert_eq!(
            path.components().next(),
            None,
            "No components should be present for empty path"
        );
    }

    #[test]
    fn test_ordering() {
        // Standard tests
        let mut paths = vec!["a/b/c/d", "a/b/c", "a/b/d", "a/b/c"];

        paths.sort();
        assert_eq!(
            paths,
            vec!["a/b/c", "a/b/c", "a/b/c/d", "a/b/d",],
            "Paths should be sorted correctly"
        );

        // These will not order correctly based on a simple string comparison since characters like
        // [!,#%] are less than the directory separator '/'

        // By lexicographical component comparison, 'b!/' comes before 'b/' but for our purposes it should come after
        let path1_special_str = "a/b!/c";
        let path2_special_str = "a/b/c";
        assert!(
            path1_special_str < path2_special_str,
            "'a/b!/c' should be less than 'a/b/c' by lexicographical comparison"
        );

        // Ensure that our RelativePath ordering handles this correctly
        let path_special1 = RelativePath::new(path1_special_str).unwrap();
        let path_special2 = RelativePath::new(path2_special_str).unwrap();
        assert!(path_special1 > path_special2, "'a/b!/c' should be greater than 'a/b/c'");
    }
}
