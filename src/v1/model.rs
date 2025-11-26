use crate::common::RelativePath;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Represents a directory in the workspace, containing its relative path and entries.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Directory {
    /// The full relative path of this directory within the workspace
    relative_path: RelativePath,
    /// The entries within this directory
    entries: Vec<DirectoryEntry>,
}

impl Directory {
    /// Creates a new Directory with the given relative path and entries
    pub fn new(relative_path: RelativePath, entries: Vec<DirectoryEntry>) -> Self {
        Directory { relative_path, entries }
    }

    /// Returns the relative path of this directory
    pub fn relative_path(&self) -> &RelativePath {
        &self.relative_path
    }

    /// Returns the entries within this directory
    pub fn entries(&self) -> &[DirectoryEntry] {
        &self.entries
    }

    pub fn push_entry(&mut self, entry: DirectoryEntry) {
        // TODO: Make sure these stay sorted and unique
        self.entries.push(entry);
    }
}

/// Represents an entry in a directory, which can be either a file or a sub-directory.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DirectoryEntry {
    name: String,
    info: DirectoryEntryType,
    conflict_state: ConflictState,
}

impl DirectoryEntry {
    /// Creates a new DirectoryEntry with the given name, type, and conflict state
    pub fn new(name: String, info: DirectoryEntryType, conflict_state: ConflictState) -> Self {
        DirectoryEntry {
            name,
            info,
            conflict_state,
        }
    }

    /// Returns the name of the directory entry
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the type information of the directory entry
    pub fn info(&self) -> &DirectoryEntryType {
        &self.info
    }

    /// Returns the conflict state of the directory entry
    pub fn conflict_state(&self) -> &ConflictState {
        &self.conflict_state
    }
}

/// The type of a directory entry, either a file or a directory.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DirectoryEntryType {
    /// The entry is a plain-old-file
    File {
        metadata: FileMetadata,
        change_state: ChangeState,
    },
    /// The entry is a directory.  If the inner value is None, the directory has not been loaded yet.
    Directory(Option<Directory>),
}

/// Metadata about a file
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FileMetadata {
    size_bytes: u64,
    modified_time_unix_ms_utc: u64,
}

impl FileMetadata {
    /// Creates a new FileMetadata with the given size and modified time
    pub fn new(size_bytes: u64, modified_time_unix_ms_utc: u64) -> Self {
        FileMetadata {
            size_bytes,
            modified_time_unix_ms_utc,
        }
    }

    /// Returns the size of the file in bytes
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns the last modified time of the file in Unix milliseconds UTC
    pub fn modified_time_unix_ms_utc(&self) -> u64 {
        self.modified_time_unix_ms_utc
    }
}

/// The change state of a directory entry, e.g. whether it is added, modified, deleted, or unchanged
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChangeState {
    /// The entry is unchanged from the base version
    #[default]
    Unchanged,
    /// The entry is new in this version
    Added,
    /// The entry has been modified in this version
    Modified,
    /// The entry has been deleted in this version
    Deleted,
}

/// The conflict state of a directory entry
/// Note, this will be updated to include metadata about the conflict, for example, who published the conflicting
/// change, timestamps, etc.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ConflictState {
    /// The entry has no conflicts
    #[default]
    None,
    /// The entry has conflicts pending resolution
    Unresolved,
    /// The entry's conflicts have been resolved
    Resolved,
    /// The entry has incoming changes that conflict with local changes
    Incoming,
}
