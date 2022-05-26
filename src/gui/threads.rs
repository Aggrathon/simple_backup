#![cfg(feature = "gui")]

use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread::JoinHandle;

use crate::backup::{BackupError, BackupWriter};
use crate::config::Config;
use crate::files::FileInfo;

pub(crate) struct ThreadWrapper<T1, T2> {
    queue: Receiver<T1>,
    handle: JoinHandle<T2>,
}

impl<T1, T2> ThreadWrapper<T1, T2> {
    pub fn try_recv(&self) -> Result<T1, TryRecvError> {
        self.queue.try_recv()
    }

    pub fn cancel(self) -> std::thread::Result<T2> {
        std::mem::drop(self.queue);
        self.handle.join()
    }

    pub fn join(self) -> std::thread::Result<T2> {
        self.handle.join()
    }
}

impl ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter> {
    pub fn crawl_for_files(config: Config) -> Self {
        let (send, queue) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let (mut writer, error) = BackupWriter::new(config);
            #[allow(unused_must_use)]
            if let Some(e) = error {
                send.send(Err(e));
            }
            let error = writer.foreach_file(true, |res| {
                send.send(match res {
                    Ok(fi) => Ok(fi.clone()),
                    Err(e) => Err(BackupError::FileAccessError(e)),
                })
                .map_err(|_| BackupError::Cancel)
            });
            #[allow(unused_must_use)]
            if let Err(e) = error {
                send.send(Err(e));
            }
            std::mem::drop(send);
            writer
        });
        Self { queue, handle }
    }

    pub fn backup_files(writer: BackupWriter) -> Self {
        let (send, queue) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut writer = writer;
            let error = writer.write(
                #[allow(unused_must_use)]
                |fi, res| {
                    if let Err(e) = res {
                        send.send(Err(e));
                    }
                    send.send(Ok(fi.clone())).map_err(|_| BackupError::Cancel)
                },
                || {},
            );
            #[allow(unused_must_use)]
            if let Err(e) = error {
                send.send(Err(e));
            }
            std::mem::drop(send);
            writer
        });
        Self { queue, handle }
    }
}
