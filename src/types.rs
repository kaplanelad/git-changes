#[derive(Debug, Clone)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
}
