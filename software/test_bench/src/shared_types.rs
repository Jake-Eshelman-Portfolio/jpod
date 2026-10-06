use std::path::PathBuf;

#[derive(Debug)]
pub struct Song {
    pub path: PathBuf,
    pub name: String,
}