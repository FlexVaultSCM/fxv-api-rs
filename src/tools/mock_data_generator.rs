// == Std
use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

// == Internal crates
use fxv_api::{
    common::RelativePath,
    v1::model::{Directory, DirectoryEntry, DirectoryEntryType, FileMetadata},
};

// == External crates
use argh::FromArgs;
use walkdir::WalkDir;

#[derive(FromArgs)]
/// Command line arguments for the mock data generator
struct Args {
    /// output compact JSON instead of pretty-printed
    #[argh(switch, short = 'c')]
    compact: bool,
    /// the target directory to serialize
    #[argh(positional)]
    target_dir: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Args = argh::from_env();

    let target_path = PathBuf::from(&args.target_dir);

    if !target_path.is_dir() {
        eprintln!("Error: target path '{}' is not a directory", args.target_dir);
        std::process::exit(1);
    } else {
        let directory = generate_directory_tree_from_path(&target_path)?;
        if args.compact {
            serde_json::to_writer(std::io::stdout(), &directory)?;
        } else {
            serde_json::to_writer_pretty(std::io::stdout(), &directory)?;
        }
    }

    Ok(())
}

/// Internal wrapper for managing a stack of directories while building the tree
/// Note: This has sharp edges and should be used with care. It is only intended for use in the
/// mock data generator, and has an invariant that there is always at least one directory in the stack until
/// it is fully popped at the end.
struct DirStack {
    stack: Vec<Directory>,
}

impl DirStack {
    fn new() -> Self {
        DirStack {
            stack: vec![Directory::new(RelativePath::new("").unwrap(), vec![])],
        }
    }

    fn last(&self) -> &Directory {
        self.stack
            .last()
            .expect("Dir stack should never call .last() when it is empty")
    }

    fn last_mut(&mut self) -> &mut Directory {
        self.stack
            .last_mut()
            .expect("Dir stack should never call .last_mut() when it is empty")
    }

    fn pop_tail(&mut self) {
        if let Some(last) = self.stack.pop() {
            if let Some(new_last) = self.stack.last_mut() {
                new_last.push_entry(DirectoryEntry::new(
                    last.relative_path().file_name().unwrap().to_string(),
                    DirectoryEntryType::Directory(Some(last)),
                ));
            }
        }
    }

    fn push_directory(&mut self, directory_path: RelativePath) {
        self.stack.push(Directory::new(directory_path, vec![]));
    }

    fn push_file(&mut self, file_name: &str, metadata: FileMetadata) {
        self.last_mut().push_entry(DirectoryEntry::new(
            file_name.to_string(),
            DirectoryEntryType::File {
                metadata,
                change_state: Default::default(),
                conflict_state: Default::default(),
            },
        ));
    }

    fn finalize(mut self) -> Directory {
        while self.stack.len() > 1 {
            self.pop_tail();
        }
        self.stack.pop().expect("There should be at least the root directory in the stack")
    }
}

fn generate_directory_tree_from_path(target_path: &Path) -> Result<Directory, Box<dyn std::error::Error>> {
    let dir_walker = WalkDir::new(target_path).sort_by_file_name();

    let mut dir_stack = DirStack::new();

    // Skip the first entry, which is the root directory itself
    for entry in dir_walker.into_iter().skip(1).filter_map(Result::ok) {
        let metadata = entry.metadata()?;
        let relative_path: RelativePath = entry
            .path()
            .strip_prefix(target_path)
            .expect("Failed to strip prefix")
            .try_into()?;

        /*println!(
            "Processing entry: {} -> {}",
            entry.path().display(),
            relative_path.as_str()
        );*/

        // Adjust the stack to the correct directory level
        let stack_path = dir_stack.last().relative_path().clone();

        let common_ancestor = stack_path.common_ancestor(&relative_path);
        while dir_stack.last().relative_path() != &common_ancestor {
            dir_stack.pop_tail();
        }

        // Create new directory if needed
        let mut missing_components = relative_path.components_starting_at_common_ancestor(&stack_path);
        while missing_components.next().is_some() {
            // Skip the file name
            if !missing_components.is_at_last_entry() {
                /*println!(
                    "Pushing new directory onto stack: {}",
                    missing_components.as_accumulated_str()
                );*/
                let new_dir_path = RelativePath::new(missing_components.as_accumulated_str())
                    .expect("Internal relative path should always be valid");
                dir_stack.push_directory(new_dir_path);
            }
        }

        // We will only push files here, directories are pushed when we pop the stack
        if !metadata.is_dir() {
            let file_name = relative_path.file_name().expect("File should have a file name");
            //println!("Pushing file: {}", file_name);
            dir_stack.push_file(
                file_name,
                FileMetadata::new(
                    metadata.len(),
                    metadata
                        .modified()
                        .expect("Should be able to get modified time")
                        .duration_since(UNIX_EPOCH)
                        .expect("Time should be after UNIX_EPOCH")
                        .as_millis() as u64,
                ),
            );
        }
    }

    Ok(dir_stack.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_directory_tree() {
        // Not the best test, but at least it verifies that the generated structure matches walkdir's output
        let target_dir = Path::new(".");
        let directory = generate_directory_tree_from_path(target_dir).expect("Failed to generate directory tree");

        let mut all_files = vec![];

        // Validate the generated directory tree
        get_all_files(&directory, &mut all_files);

        // Get the same from walkdir for comparison
        let walkdir_files = WalkDir::new(target_dir)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|e| {
                if e.metadata().unwrap().is_dir() {
                    None
                } else {
                    Some(e.path().strip_prefix(target_dir).unwrap().to_string_lossy().to_string())
                }
            })
            .collect::<Vec<_>>();

        assert_eq!(
            all_files.len(),
            walkdir_files.len(),
            "Number of files should match between generated directory tree and walkdir"
        );

        all_files
            .iter()
            .zip(walkdir_files.iter())
            .enumerate()
            .for_each(|(i, (gen_file, wd_file))| {
                assert_eq!(
                    gen_file.as_str(),
                    wd_file,
                    "File paths should match: (index {}) {} vs {}",
                    i,
                    gen_file.as_str(),
                    wd_file
                );
            });
    }

    fn get_all_files(directory: &Directory, all_files: &mut Vec<RelativePath>) {
        for entry in directory.entries() {
            match entry.info() {
                DirectoryEntryType::Directory(Some(sub_dir)) => {
                    get_all_files(sub_dir, all_files);
                }
                DirectoryEntryType::Directory(None) => {
                    // Empty directory, do nothing
                }
                DirectoryEntryType::File { .. } => {
                    let file_path = if directory.relative_path().as_str().is_empty() {
                        entry.name().to_string()
                    } else {
                        format!("{}/{}", directory.relative_path().as_str(), entry.name())
                    };
                    all_files.push(RelativePath::new(&file_path).unwrap());
                }
            }
        }
    }
}
