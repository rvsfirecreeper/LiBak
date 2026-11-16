use super::Error;
use fastcdc::v2020;
use std::{
    fs::{self, File},
    path::Path,
};
fn scan(path: &Path, exclude: &[&Path]) -> Result<Vec<File>, Error> {
    if !path.try_exists()? {
        return Err(Error::NonExistent);
    }
    let mut files = vec![];
    if path.is_dir() {
        for dir_entry in fs::read_dir(path)? {
            let dir_entry = dir_entry?;
            if !exclude.iter().any(|i| *i == dir_entry.path().as_path()) {
                files.push(File::open(dir_entry.path())?);
            }
        }
    } else if !exclude.iter().any(|i| *i == path) {
        files.push(File::open(path)?);
    }

    Ok(files)
}
