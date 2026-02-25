use std::{fs, io::Read, path, thread, time};

use crate::filesystem::ResourceLock;

pub struct BasicMessageConsumer {
    queue_directory: path::PathBuf,
    lockfile_path: path::PathBuf,
    registry_path: path::PathBuf,
    // queue_name: String,
}

impl BasicMessageConsumer {
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

    fn acquire_lock(&self) -> ResourceLock {
        return ResourceLock::new(&self.lockfile_path);
    }

    fn pop_registry(&self) -> Option<String> {
        let contents = fs::read_to_string(&self.registry_path).unwrap();

        let message_id =
            contents
                .match_indices('\n')
                .into_iter()
                .next()
                .map(|(first_newline, _)| {
                    let (first_line, _) = contents.split_at(first_newline);
                    let (_, remainder) = contents.split_at(first_newline + 1);

                    fs::write(&self.registry_path, remainder).unwrap();

                    String::from(first_line)
                });

        message_id
    }
    pub fn consume_one_message(&self) -> Option<Vec<u8>> {
        let _lock = self.acquire_lock();

        println!("acquired lock");
        thread::sleep(time::Duration::from_secs(10));

        let message_id = self.pop_registry();
        message_id.map(|message_id| {
            let mut message_path = self.queue_directory.clone();
            message_path.push(&message_id);

            let f = fs::File::open(&message_path).unwrap();
            let contents = f.bytes().map(|x| x.unwrap()).collect();

            let mut new_message_path = self.queue_directory.clone();
            new_message_path.push("read");
            new_message_path.push(&message_id);
            _ = fs::rename(message_path, new_message_path).unwrap();

            contents
        })
    }
}
