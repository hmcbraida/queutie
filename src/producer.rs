use std::{fs, io::Write, path};
use uuid::Uuid;

use crate::filesystem::ResourceLock;

pub struct BasicMessageProducer {
    queue_directory: path::PathBuf,
    lockfile_path: path::PathBuf,
    registry_path: path::PathBuf,
}

impl BasicMessageProducer {
    pub fn new<T>(root_directory: T, queue_name: impl AsRef<str>) -> Self
    where
        path::PathBuf: From<T>,
    {
        let queue_name = String::from(queue_name.as_ref());
        let mut queue_directory = path::PathBuf::from(root_directory);
        queue_directory.push(&queue_name);

        let mut lockfile_path = queue_directory.clone();
        lockfile_path.push(".lock");

        let mut registry_path = queue_directory.clone();
        registry_path.push("registry.txt");

        return Self {
            queue_directory,
            lockfile_path,
            registry_path,
        };
    }

    fn push_registry(&self, message_id: impl AsRef<str>) {
        let _lock = ResourceLock::new(&self.lockfile_path);

        let mut file = fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.registry_path)
            .unwrap();

        writeln!(file, "{}", message_id.as_ref()).unwrap();
    }

    pub fn push_to_queue<T: AsRef<[u8]>>(&self, message: T) {
        let message_id = Uuid::new_v4().to_string();

        let mut message_path = self.queue_directory.clone();
        message_path.push(&message_id);

        // Give the file creation its own scope to ensure it is read
        {
            let _f = fs::File::create(message_path)
                .unwrap()
                .write_all(message.as_ref())
                .unwrap();
        }

        self.push_registry(&message_id);
    }
}
