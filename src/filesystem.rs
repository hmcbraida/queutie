use std::{fs, path};

pub struct ResourceLock {
    lock_file: fs::File,
}

impl ResourceLock {
    pub fn new(lockfile_path: impl AsRef<path::Path>) -> Self {
        let lock_file = fs::File::create(lockfile_path).unwrap();
        lock_file.lock().unwrap();

        Self { lock_file }
    }
}

impl Drop for ResourceLock {
    fn drop(&mut self) {
        self.lock_file.unlock().unwrap();
    }
}
