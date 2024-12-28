#![cfg(feature = "gui")]

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread::JoinHandle;

use crate::backup::{BackupError, BackupMerger, BackupReader, BackupWriter};
use crate::config::Config;
use crate::files::FileInfo;
use crate::utils::strip_absolute_from_path;

pub(crate) struct ThreadWrapper<T1, T2> {
    batch_size: usize,
    batch_mult: usize,
    index: usize,
    queue: Option<Receiver<T1>>,
    handle: JoinHandle<T2>,
}

impl<T1, T2> ThreadWrapper<T1, T2> {
    pub fn try_recv(&self) -> Result<T1, TryRecvError> {
        if let Some(q) = &self.queue {
            q.try_recv()
        } else {
            Err(TryRecvError::Disconnected)
        }
    }

    pub fn try_cancel(&mut self) -> bool {
        std::mem::drop(self.queue.take());
        self.handle.is_finished()
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
    pub fn crawl_for_files(config: Config, batch_size: usize) -> Self {
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
        Self {
            batch_size,
            batch_mult: 1,
            index: 0,
            queue: Some(queue),
            handle,
        }
    }

    pub fn backup_files(writer: BackupWriter, batch_size: usize) -> Self {
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
        Self {
            batch_size,
            batch_mult: 1,
            index: 0,
            queue: Some(queue),
            handle,
        }
    }
}

impl ThreadWrapper<Result<FileInfo, BackupError>, BackupMerger> {
    pub fn merge_backups(merger: BackupMerger, batch_size: usize) -> Self {
        let (send, queue) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut merger = merger;
            let error = merger.write(
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
            merger
        });
        Self {
            batch_size,
            batch_mult: 1,
            index: 0,
            queue: Some(queue),
            handle,
        }
    }
}

impl ThreadWrapper<Result<FileInfo, BackupError>, BackupReader> {
    pub fn restore_files(
        reader: BackupReader,
        selection: Vec<String>,
        flatten: bool,
        output: Option<PathBuf>,
        batch_size: usize,
    ) -> Result<Self, (BackupReader, BackupError)> {
        if flatten && output.is_none() {
            return Err((
                reader,
                BackupError::GenericError("The output must be given if flatten=true"),
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
                reader.restore(selection, path_transform, callback, true, true)
            } else {
                let path_transform = |mut fi: FileInfo| match &output {
                    Some(output) => {
                        FileInfo::from(output.join(strip_absolute_from_path(&fi.move_string())))
                    }
                    None => fi,
                };
                reader.restore(selection, path_transform, callback, true, true)
            };

            #[allow(unused_must_use)]
            if let Err(e) = error {
                send.send(Err(e));
            }
            std::mem::drop(send);
            reader
        });
        Ok(Self {
            batch_size,
            batch_mult: 1,
            index: 0,
            queue: Some(queue),
            handle,
        })
    }
}

/// Iterator over batches that tries to scale the batch size to match the queue size
impl<T1, T2> Iterator for ThreadWrapper<T1, T2> {
    type Item = Result<T1, TryRecvError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.batch_size * self.batch_mult {
            self.batch_mult *= 2;
            self.index = 0;
            None
        } else {
            self.index += 1;
            match self.try_recv() {
                Ok(v) => Some(Ok(v)),
                Err(e) => match e {
                    TryRecvError::Empty => {
                        self.batch_mult = self.batch_mult / 2 + 1;
                        self.index = 0;
                        None
                    }
                    TryRecvError::Disconnected => Some(Err(e)),
                },
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.batch_size))
    }
}
