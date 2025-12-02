// == Std
use std::{
    fmt::Display,
    iter::FusedIterator,
    path::{Path, PathBuf},
};

// == External crates
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with RelativePath
#[derive(Debug, Clone, Error)]
pub enum RelativePathError {
    #[error("The provided path '{0}' is invalid as a relative path")]
    InvalidPath(String),
    #[error("Failed to convert OS path: {0}")]
    OsPathConversionError(PathBuf),
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

    /// Returns the string representation of the relative path
    pub fn as_str(&self) -> &str {
        &self.0
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
        RelativePathComponents {
            inner: &self.0,
            index: 0,
        }
    }

    /// Returns true if the relative path is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the common ancestor of this path and another path
    /// For example, the common ancestor of "a/b/c/d" and "a/b/e/f" is "a/b"
    /// The common ancestor of "a/b/c" and "d/e/f" is the empty root path
    pub fn common_ancestor<'a>(&'a self, other: &RelativePath) -> RelativePathComponents<'a> {
        RelativePathComponents {
            inner: &self.0[..self.common_ancestor_separator_index(other)],
            index: 0,
        }
    }

    /// Returns the components iterator of this path starting at the common ancestor with another path
    /// For example, for self of "a/b/c/d" compared with "a/b/e/f", this will return an iterator over "a/b/c/d" already
    /// advanced to "c"
    pub fn components_starting_at_common_ancestor<'a>(&'a self, other: &RelativePath) -> RelativePathComponents<'a> {
        let index = self.common_ancestor_separator_index(other);
        RelativePathComponents {
            inner: &self.0,
            index: index + 1,
        }
    }

    /// Returns the common ancestor of this path and another path, along with the remainder of the other path
    fn common_ancestor_separator_index(&self, other: &RelativePath) -> usize {
        let mut self_iter = self.components();
        let mut other_iter = other.components();

        let mut index = 0;
        while self_iter.next().is_some_and(|s| Some(s) == other_iter.next()) {
            index = self_iter.index;
        }

        index.saturating_sub(1)
    }

    /// Replaces all backslashes in the path with forward slashes if they exist
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

impl<'a> PartialEq<RelativePathComponents<'a>> for RelativePath {
    fn eq(&self, other: &RelativePathComponents<'a>) -> bool {
        self.0 == other.as_full_str()
    }
}

impl PartialEq<str> for RelativePath {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl TryFrom<&Path> for RelativePath {
    type Error = RelativePathError;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        // Convert Path to utf-8
        if let Some(path_str) = value.to_str() {
            RelativePath::new(path_str)
        } else {
            Err(RelativePathError::OsPathConversionError(value.to_path_buf()))
        }
    }
}

/// An iterator over the components of a RelativePath
#[derive(Debug, Clone)]
pub struct RelativePathComponents<'a> {
    inner: &'a str,
    index: usize,
}

impl<'a> RelativePathComponents<'a> {
    /// Returns the full path string represented by this iterator, constant over the iterator state
    pub fn as_full_str(&self) -> &'a str {
        self.inner
    }

    /// Returns the accumulated path string up to (but not including) the current component
    pub fn as_accumulated_str(&self) -> &'a str {
        &self.inner[..self.index.saturating_sub(1)]
    }

    pub fn is_at_last_entry(&self) -> bool {
        self.index >= self.inner.len()
    }
}

impl<'a> Iterator for RelativePathComponents<'a> {
    type Item = &'a str;

    /// Returns the next component of the relative path, or None if there are no more components
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.inner.len() {
            None
        } else {
            let next_index = self.inner[self.index..]
                .find('/')
                .unwrap_or(self.inner.len() - self.index);
            let component = &self.inner[self.index..self.index + next_index];
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

    #[test]
    fn test_common_ancestor() {
        let path1 = RelativePath::new("a/b/c/d").unwrap();
        let path2 = RelativePath::new("a/b/e/f").unwrap();
        let common_ancestor = path1.common_ancestor(&path2);
        assert_eq!(common_ancestor.as_full_str(), "a/b", "Common ancestor should be 'a/b'");

        // Test components starting at common ancestor
        let mut common_ancestor_split = path1.components_starting_at_common_ancestor(&path2);
        assert_eq!(
            common_ancestor_split.as_full_str(),
            "a/b/c/d",
            "Remainder path should be 'a/b/c/d'"
        );
        assert_eq!(
            common_ancestor_split.as_accumulated_str(),
            "a/b",
            "Accumulated path should be 'a/b'"
        );
        assert_eq!(common_ancestor_split.next(), Some("c"), "Next component should be 'c'");
        assert_eq!(
            common_ancestor_split.as_accumulated_str(),
            "a/b/c",
            "Accumulated path should be 'a/b/c'"
        );
        assert_eq!(common_ancestor_split.next(), Some("d"), "Next component should be 'd'");
        assert_eq!(
            common_ancestor_split.as_accumulated_str(),
            "a/b/c/d",
            "Accumulated path should be 'a/b/c/d'"
        );
        assert_eq!(
            common_ancestor_split.next(),
            None,
            "No more components should be present"
        );

        let path1 = RelativePath::new("a/b/c").unwrap();
        let path2 = RelativePath::new("a/b/c/d/e").unwrap();
        let common_ancestor = path1.common_ancestor(&path2);
        assert_eq!(
            common_ancestor.as_full_str(),
            "a/b/c",
            "Common ancestor should be 'a/b/c'"
        );

        let path1 = RelativePath::new("a/b/c").unwrap();
        let path2 = RelativePath::new("d/e/f").unwrap();
        let common_ancestor = path1.common_ancestor(&path2);
        assert_eq!(
            common_ancestor.as_full_str(),
            "",
            "Common ancestor should be empty string"
        );
    }

    #[test]
    fn test_iterator_str() {
        let relative_path = RelativePath::new("a/b/c/d/e.txt").unwrap();
        let mut components = relative_path.components();
        assert_eq!(components.as_full_str(), "a/b/c/d/e.txt");
        assert_eq!(components.as_accumulated_str(), "");
        assert_eq!(components.next(), Some("a"));
        assert_eq!(components.as_accumulated_str(), "a");
        assert_eq!(components.next(), Some("b"));
        assert_eq!(components.as_accumulated_str(), "a/b");
        assert_eq!(components.next(), Some("c"));
        assert_eq!(components.as_accumulated_str(), "a/b/c");
        assert_eq!(components.next(), Some("d"));
        assert_eq!(components.as_accumulated_str(), "a/b/c/d");
        assert_eq!(components.next(), Some("e.txt"));
        assert_eq!(components.as_accumulated_str(), "a/b/c/d/e.txt");
        assert_eq!(components.next(), None);
        assert_eq!(components.as_accumulated_str(), "a/b/c/d/e.txt");
    }
}
