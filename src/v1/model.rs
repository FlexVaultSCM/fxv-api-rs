// == Std

// == Internal crates
use crate::common::RelativePath;

// == External crates
use enumset::{EnumSet, EnumSetType};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub type ChangeStateSet = EnumSet<ChangeState>;
pub type ConflictStateSet = EnumSet<ConflictState>;

/// Represents a directory in the workspace, containing its relative path and entries.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Directory {
    /// The full relative path of this directory within the workspace
    relative_path: RelativePath,
    /// The entries within this directory
    entries: Vec<DirectoryEntry>,
    /// The aggregated union of conflict states of all entries within this directory
    conflict_states: ConflictStateSet,
    /// The aggregated union of change states of all entries within this directory
    change_states: ChangeStateSet,
}

impl Directory {
    /// Creates a new Directory with the given relative path and entries
    pub fn new(relative_path: RelativePath, entries: Vec<DirectoryEntry>) -> Self {
        // Aggregate the child conflict states and change states
        let (conflict_states, change_states) = entries.iter().fold(
            (ConflictStateSet::default(), ChangeStateSet::default()),
            |(mut conflicts, mut changes), entry| {
                entry.aggregate_states_into(&mut conflicts, &mut changes);
                (conflicts, changes)
            },
        );
        Directory {
            relative_path,
            entries,
            conflict_states,
            change_states,
        }
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
        entry.aggregate_states_into(&mut self.conflict_states, &mut self.change_states);
        self.entries.push(entry);
    }

    /// Prunes (unloads, i.e. sets to None) directory sub-entries beyond the specified depth limit
    pub fn prune_to_depth(&mut self, depth_limit: u32) {
        for entry in &mut self.entries {
            if let DirectoryEntryType::Directory(Some(dir)) = &mut entry.info {
                if depth_limit > 0 {
                    dir.prune_to_depth(depth_limit - 1);
                } else {
                    // Depth limit reached, unload this directory
                    entry.info = DirectoryEntryType::Directory(None);
                }
            }
        }
    }
}

/// Represents an entry in a directory, which can be either a file or a sub-directory.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DirectoryEntry {
    name: String,
    info: DirectoryEntryType,
}

impl DirectoryEntry {
    /// Creates a new DirectoryEntry with the given name and type
    pub fn new(name: String, info: DirectoryEntryType) -> Self {
        DirectoryEntry { name, info }
    }

    /// Returns the name of the directory entry
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the type information of the directory entry
    pub fn info(&self) -> &DirectoryEntryType {
        &self.info
    }

    pub(crate) fn aggregate_states_into(
        &self,
        conflict_states: &mut ConflictStateSet,
        change_states: &mut ChangeStateSet,
    ) {
        match &self.info {
            DirectoryEntryType::File {
                conflict_state,
                change_state,
                ..
            } => {
                conflict_states.insert(*conflict_state);
                change_states.insert(*change_state);
            }
            DirectoryEntryType::Directory(Some(dir)) => {
                conflict_states.insert_all(dir.conflict_states);
                change_states.insert_all(dir.change_states);
            }
            DirectoryEntryType::Directory(None) => {
                // Unloaded directory, do nothing
            }
        }
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
        conflict_state: ConflictState,
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
#[derive(Default, Debug, Hash, EnumSetType)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", enumset(serialize_repr = "list"))]
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
#[derive(Default, Debug, Hash, EnumSetType)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", enumset(serialize_repr = "list"))]
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

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_state_aggregation() {
        let file1 = DirectoryEntry::new(
            "file1.txt".into(),
            DirectoryEntryType::File {
                metadata: FileMetadata::new(100, 1620000000000),
                change_state: ChangeState::Added,
                conflict_state: ConflictState::None,
            },
        );

        let file2 = DirectoryEntry::new(
            "file2.txt".into(),
            DirectoryEntryType::File {
                metadata: FileMetadata::new(200, 1620000001000),
                change_state: ChangeState::Modified,
                conflict_state: ConflictState::Unresolved,
            },
        );

        let sub_dir = Directory::new(RelativePath::new("subdir").unwrap(), vec![file2.clone()]);

        let dir = Directory::new(
            RelativePath::new("").unwrap(),
            vec![
                file1.clone(),
                DirectoryEntry::new("subdir".into(), DirectoryEntryType::Directory(Some(sub_dir.clone()))),
            ],
        );

        assert!(dir.change_states.contains(ChangeState::Added));
        assert!(dir.change_states.contains(ChangeState::Modified));
        assert!(!dir.change_states.contains(ChangeState::Deleted));

        assert!(dir.conflict_states.contains(ConflictState::None));
        assert!(dir.conflict_states.contains(ConflictState::Unresolved));
        assert!(!dir.conflict_states.contains(ConflictState::Resolved));

        // Ensure that the same holds for push_entry
        let mut dir2 = Directory::new(RelativePath::new("").unwrap(), vec![]);
        dir2.push_entry(file1);
        dir2.push_entry(DirectoryEntry {
            name: "subdir".into(),
            info: DirectoryEntryType::Directory(Some(sub_dir)),
        });
        assert_eq!(dir.change_states, dir2.change_states);
        assert_eq!(dir.conflict_states, dir2.conflict_states);
    }

    #[test]
    fn test_pruning() {
        let mut root_dir_entry = DirectoryEntry::new(
            "".into(),
            DirectoryEntryType::Directory(Some(Directory::new(RelativePath::new("").unwrap(), vec![]))),
        );

        push_entry(&mut root_dir_entry, new_file("file_root.txt"));

        // Build a dir structure like:
        // file_root.txt
        // subdir_a_l1/
        //   subdir_a_l2/
        //     subdir_a_l3/
        //       subdir_a_l4/
        //         file_d.txt
        // subdir_b_l1/
        //   file_b.txt
        //   subdir_b_l2/
        //     file_c.txt
        let mut subdir_a_l1 = new_dir(&root_dir_entry, "subdir_a_l1");
        let mut subdir_a_l2 = new_dir(&subdir_a_l1, "subdir_a_l2");
        let mut subdir_a_l3 = new_dir(&subdir_a_l2, "subdir_a_l3");
        let mut subdir_a_l4 = new_dir(&subdir_a_l3, "subdir_a_l4");

        push_entry(&mut subdir_a_l4, new_file("file_d.txt"));
        push_entry(&mut subdir_a_l3, subdir_a_l4);
        push_entry(&mut subdir_a_l2, subdir_a_l3);
        push_entry(&mut subdir_a_l1, subdir_a_l2);
        push_entry(&mut root_dir_entry, subdir_a_l1);

        let mut subdir_b_l1 = new_dir(&root_dir_entry, "subdir_b_l1");
        push_entry(&mut subdir_b_l1, new_file("file_b.txt"));
        let mut subdir_b_l2 = new_dir(&subdir_b_l1, "subdir_b_l2");
        push_entry(&mut subdir_b_l2, new_file("file_c.txt"));
        push_entry(&mut subdir_b_l1, subdir_b_l2);
        push_entry(&mut root_dir_entry, subdir_b_l1);

        let root_directory = match &mut root_dir_entry.info {
            DirectoryEntryType::Directory(Some(dir)) => dir,
            _ => panic!("Root should be a directory"),
        };

        let mut names = vec![];
        collect_names(root_directory, &mut names);
        assert_eq!(
            names,
            vec![
                "file_root.txt",
                "subdir_a_l1",
                "subdir_a_l1/subdir_a_l2",
                "subdir_a_l1/subdir_a_l2/subdir_a_l3",
                "subdir_a_l1/subdir_a_l2/subdir_a_l3/subdir_a_l4",
                "subdir_a_l1/subdir_a_l2/subdir_a_l3/subdir_a_l4/file_d.txt",
                "subdir_b_l1",
                "subdir_b_l1/file_b.txt",
                "subdir_b_l1/subdir_b_l2",
                "subdir_b_l1/subdir_b_l2/file_c.txt",
            ]
        );

        // Prune to depth 3
        // This should remove subdir_a_l4 and its contents, but keep everything else
        // subdir_a_l4 SHOULD still be in the list, but its contents should be gone
        root_directory.prune_to_depth(3);
        names.clear();
        collect_names(root_directory, &mut names);
        assert_eq!(
            names,
            vec![
                "file_root.txt",
                "subdir_a_l1",
                "subdir_a_l1/subdir_a_l2",
                "subdir_a_l1/subdir_a_l2/subdir_a_l3",
                // This directory should still be present, but it should be unloaded
                "subdir_a_l1/subdir_a_l2/subdir_a_l3/subdir_a_l4 (unloaded)",
                "subdir_b_l1",
                "subdir_b_l1/file_b.txt",
                "subdir_b_l1/subdir_b_l2",
                // Ensure that files at the prune depth are still present
                "subdir_b_l1/subdir_b_l2/file_c.txt",
            ]
        );

        // Prune to depth 1
        // This should remove subdir_a_l2 and everything under it, and subdir_b_l2 and its contents
        root_directory.prune_to_depth(1);
        names.clear();
        collect_names(root_directory, &mut names);
        assert_eq!(
            names,
            vec![
                "file_root.txt",
                "subdir_a_l1",
                // This directory should still be present, but it should be unloaded
                "subdir_a_l1/subdir_a_l2 (unloaded)",
                "subdir_b_l1",
                "subdir_b_l1/file_b.txt",
                // This directory should still be present, but it should be unloaded
                "subdir_b_l1/subdir_b_l2 (unloaded)",
            ]
        );

        // Try pruning to depth 0
        root_directory.prune_to_depth(0);
        names.clear();
        collect_names(root_directory, &mut names);
        assert_eq!(
            names,
            vec!["file_root.txt", "subdir_a_l1 (unloaded)", "subdir_b_l1 (unloaded)",]
        );
    }

    fn collect_names(dir: &Directory, names: &mut Vec<String>) {
        for entry in &dir.entries {
            // We annotated unloaded directories specially
            let mut full_path_string = dir.relative_path().try_join(&entry.name).unwrap().to_string();
            if matches!(&entry.info, DirectoryEntryType::Directory(None)) {
                full_path_string.push_str(" (unloaded)");
            }

            names.push(full_path_string);
            if let DirectoryEntryType::Directory(Some(sub_dir)) = &entry.info {
                collect_names(sub_dir, names);
            }
        }
    }

    fn new_dir(parent: &DirectoryEntry, name: &str) -> DirectoryEntry {
        let parent_path = match &parent.info {
            DirectoryEntryType::Directory(Some(dir)) => dir.relative_path(),
            _ => panic!("Parent must be a directory"),
        };

        let relative_path = parent_path.try_join(name).unwrap();

        DirectoryEntry::new(
            relative_path.file_name().expect("Should have a file name").to_string(),
            DirectoryEntryType::Directory(Some(Directory::new(relative_path, vec![]))),
        )
    }

    fn push_entry(dir: &mut DirectoryEntry, entry: DirectoryEntry) {
        if let DirectoryEntryType::Directory(Some(directory)) = &mut dir.info {
            directory.push_entry(entry);
        } else {
            panic!("DirectoryEntry is not a directory");
        }
    }

    fn new_file(name: &str) -> DirectoryEntry {
        DirectoryEntry::new(
            name.to_string(),
            DirectoryEntryType::File {
                metadata: FileMetadata::new(0, 0),
                change_state: ChangeState::default(),
                conflict_state: ConflictState::default(),
            },
        )
    }
}
