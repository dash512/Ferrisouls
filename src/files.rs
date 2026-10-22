//! Module for certain handling of directories and files

use std::path::PathBuf;


pub fn get_files_by(
    dir: PathBuf,
    f: impl Fn(&str) -> bool
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for file in dir.read_dir().expect("failed to read directory!") {
        if let Ok(file) = file {
            let path = file.path();
            let strpath = match path.to_str() {
                Some(value) => value,
                None => continue
            };

            if f(strpath) {
                files.push(path);
            }
        }
    }

    files
}