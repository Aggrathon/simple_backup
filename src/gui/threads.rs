#![cfg(feature = "gui")]

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread::JoinHandle;

use crate::backup::{BackupError, BackupReader, BackupWriter};
use crate::config::Config;
use crate::files::FileInfo;
use crate::utils::strip_absolute_from_path;

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

impl ThreadWrapper<Result<FileInfo, BackupError>, BackupReader> {
    pub fn restore_files(
        reader: BackupReader,
        selection: Vec<String>,
        flatten: bool,
        output: Option<PathBuf>,
    ) -> Result<Self, BackupError> {
        if flatten && output.is_none() {
            return Err(BackupError::GenericError(
                "The output must be given if flatten=true",
            ));
        }

        let (send, queue) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut reader = reader;

            let callback = |res: std::io::Result<FileInfo>| {
                match res {
                    Ok(fi) => send.send(Ok(fi)),
                    Err(e) => send.send(Err(BackupError::IOError(e))),
                }
                .map_err(|_| BackupError::Cancel)
            };

            let error = if flatten {
                let output = output.unwrap();
                let path_transform = |fi: FileInfo| {
                    FileInfo::from(output.join(fi.consume_path().file_name().unwrap()))
                };
                reader.restore_selected(selection, path_transform, callback, true)
            } else {
                let path_transform = |mut fi: FileInfo| match &output {
                    Some(output) => {
                        FileInfo::from(output.join(strip_absolute_from_path(&fi.move_string())))
                    }
                    None => fi,
                };
                reader.restore_selected(selection, path_transform, callback, true)
            };

            #[allow(unused_must_use)]
            if let Err(e) = error {
                send.send(Err(e));
            }
            std::mem::drop(send);
            reader
        });
        Ok(Self { queue, handle })
    }
}
