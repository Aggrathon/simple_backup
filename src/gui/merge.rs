#![cfg(feature = "gui")]

use std::path::{Path, PathBuf};

use iced::alignment::Horizontal;
use iced::widget::Space;
use iced::{Element, Length, Subscription};
use rfd::FileDialog;

use super::threads::ThreadWrapper;
use super::{presets, Message};
use crate::backup::{BackupError, BackupMerger, BackupReader, BACKUP_FILE_EXTENSION};
use crate::files::FileInfo;
use crate::utils::{default_dir, default_dir_opt};

fn open_backups<P: AsRef<Path>>(dir: Option<P>) -> Option<Vec<PathBuf>> {
    if let Some(dir) = dir {
        FileDialog::new()
            .set_directory(dir)
            .set_title("Open backup files")
            .add_filter("Backup files", &[&BACKUP_FILE_EXTENSION[1..]])
            .pick_files()
    } else {
        open_backups(Some(default_dir()))
    }
}

fn select_output<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    let mut fd = FileDialog::new();
    if let Some(parent) = path.as_ref().parent() {
        fd = fd.set_directory(parent);
    } else if let Some(parent) = default_dir_opt() {
        fd = fd.set_directory(parent);
    }
    if let Some(name) = path.as_ref().file_name() {
        fd = fd.set_file_name(name.to_string_lossy());
    }
    fd.set_title("Where should the merged backup be stored")
        .add_filter("Backup files", &[&BACKUP_FILE_EXTENSION[1..]])
        .pick_file()
}

enum MergeStage {
    Failed,
    Selecting(Vec<BackupReader>),
    Performing(ThreadWrapper<Result<FileInfo, BackupError>, BackupMerger>),
    Cancelling(ThreadWrapper<Result<FileInfo, BackupError>, BackupMerger>),
    Error,
    Cancelled,
    Completed,
}

pub(crate) struct MergeState {
    error: String,
    total_count: usize,
    current_count: usize,
    all: bool,
    delete: bool,
    quality: Option<i32>,
    threads: Option<u32>,
    thread_alt: Vec<u32>,
    compression_alt: Vec<i32>,
    stage: MergeStage,
}

impl MergeState {
    pub fn new() -> Self {
        Self {
            error: String::new(),
            total_count: 0,
            current_count: 0,
            all: false,
            delete: false,
            quality: None,
            threads: None,
            thread_alt: (1..=num_cpus::get() as u32).collect(),
            compression_alt: (1..=22).collect(),
            stage: MergeStage::Selecting(Vec::new()),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tick => match &mut self.stage {
                MergeStage::Performing(wrapper) => {
                    for recv in wrapper {
                        match recv {
                            Ok(res) => match res {
                                Ok(_) => {
                                    self.current_count += 1;
                                }
                                Err(e) => {
                                    self.error.push('\n');
                                    self.error.push_str(&e.to_string());
                                }
                            },
                            Err(e) => match e {
                                std::sync::mpsc::TryRecvError::Empty => {
                                    break;
                                }
                                std::sync::mpsc::TryRecvError::Disconnected => {
                                    if let MergeStage::Performing(wrapper) =
                                        std::mem::replace(&mut self.stage, MergeStage::Failed)
                                    {
                                        match wrapper.join() {
                                            Ok(_) => {
                                                self.current_count = 0;
                                                self.stage = MergeStage::Completed;
                                            }
                                            Err(_) => {
                                                self.error.push_str(
                                                    "\nFailure when finalising the backup",
                                                );
                                            }
                                        }
                                    }
                                    break;
                                }
                            },
                        }
                    }
                }
                MergeStage::Cancelling(wrapper) => {
                    if wrapper.try_cancel() {
                        if let MergeStage::Cancelling(wrapper) =
                            std::mem::replace(&mut self.stage, MergeStage::Failed)
                        {
                            match wrapper.cancel() {
                                Ok(merger) => {
                                    if let Err(e) = merger.delete_file() {
                                        self.error.push('\n');
                                        self.error.push_str(&e.to_string());
                                    }
                                    self.current_count = 0;
                                    self.stage = MergeStage::Cancelled;
                                }
                                Err(_) => {
                                    self.error.push_str("\nFailure when cancelling the backup");
                                }
                            };
                        }
                    }
                }
                _ => {}
            },
            Message::Merge => {
                if let MergeStage::Selecting(_) = &self.stage {
                    if let MergeStage::Selecting(list) =
                        std::mem::replace(&mut self.stage, MergeStage::Failed)
                    {
                        match BackupMerger::new(
                            None,
                            list,
                            self.all,
                            self.delete,
                            true,
                            self.quality,
                            self.threads,
                        ) {
                            Ok(mut merger) => {
                                if let Some(path) = select_output(&merger.path) {
                                    merger.path = path;
                                    self.current_count = merger.files.len();
                                    self.stage = MergeStage::Performing(
                                        ThreadWrapper::merge_backups(merger, 1000),
                                    );
                                } else {
                                    self.stage = MergeStage::Selecting(merger.deconstruct());
                                }
                            }
                            Err((_, e)) => {
                                self.error.push('\n');
                                self.error.push_str(&e.to_string());
                                self.stage = MergeStage::Error;
                            }
                        }
                    }
                }
            }
            Message::Cancel => {
                if let MergeStage::Performing(_) = &self.stage {
                    if let MergeStage::Performing(wrapper) =
                        std::mem::replace(&mut self.stage, MergeStage::Failed)
                    {
                        self.stage = MergeStage::Cancelling(wrapper);
                    }
                }
            }
            Message::IncludeRemove(i) => {
                if let MergeStage::Selecting(list) = &mut self.stage {
                    list.remove(i);
                }
            }
            Message::IncludeAdd(_) => {
                if let MergeStage::Selecting(list) = &mut self.stage {
                    let dir = list.iter_mut().next().map(|r| r.path.get_path());
                    let open = open_backups(dir);
                    if let Some(list2) = open {
                        for p in list2.into_iter() {
                            let mut reader = BackupReader::new(p);
                            if let Err(e) = reader.get_meta() {
                                self.error.push('\n');
                                self.error.push_str(&e.to_string());
                            } else if !list.iter().any(|r| r.path == reader.path) {
                                match reader.get_meta() {
                                    Ok((config, _)) => {
                                        if list.is_empty() {
                                            if self.quality.is_none() {
                                                self.quality = Some(config.quality);
                                            }
                                            if self.threads.is_none() {
                                                self.threads = Some(config.threads);
                                            }
                                        }
                                        list.push(reader);
                                    }
                                    Err(e) => {
                                        self.error.push('\n');
                                        self.error.push_str(&e.to_string());
                                    }
                                }
                            }
                        }
                    };
                }
            }
            Message::All(b) => {
                self.all = b;
            }
            Message::Delete(b) => {
                self.delete = b;
            }
            Message::CompressionQuality(q) => {
                self.quality = Some(q);
            }
            Message::ThreadCount(n) => {
                self.threads = Some(n);
            }
            Message::Repeat => {
                *self = Self::new();
            }
            _ => eprintln!("Unexpected GUI message: {:?}", message),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self.stage {
            MergeStage::Performing(_) => {
                iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::Tick)
            }
            MergeStage::Cancelling(_) => {
                iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick)
            }
            _ => Subscription::none(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let mut scroll = presets::column_list();
        if !self.error.is_empty() {
            scroll = scroll.push(presets::text_error(&self.error[1..]));
        }
        match &self.stage {
            MergeStage::Selecting(list) => {
                scroll = list.iter().enumerate().fold(scroll, |s, (i, r)| {
                    s.push(presets::row_list2(vec![
                        presets::button_icon("-", Message::IncludeRemove(i), true),
                        presets::text(r.path.copy_string())
                            .width(Length::Fill)
                            .align_x(Horizontal::Left)
                            .into(),
                    ]))
                });
                scroll = scroll.push(presets::space_large());
                scroll = scroll.push(presets::row_list2(vec![
                    presets::space_hfill(),
                    presets::button("  Add backup  ", Message::IncludeAdd(0)),
                    presets::space_hfill(),
                ]));
                let mess = if list.len() < 2 {
                    Message::None
                } else {
                    Message::Merge
                };
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false),
                    Space::with_width(Length::Fill).into(),
                    presets::text("Compression:").into(),
                    presets::pick_list(
                        &self.compression_alt,
                        self.quality,
                        Message::CompressionQuality,
                    ),
                    presets::space_large(),
                    presets::text("Threads:").into(),
                    presets::pick_list(&self.thread_alt, self.threads, Message::ThreadCount),
                    presets::space_large(),
                    presets::toggler(self.all, "Deleted files", Message::All),
                    presets::space_large(),
                    presets::toggler_comp(self.delete, "Remove merged", Message::Delete),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav("Merge", mess, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            MergeStage::Performing(_) | MergeStage::Cancelling(_) => {
                let status = if let MergeStage::Cancelling(_) = self.stage {
                    presets::text_center_error("Cancelling the merging...")
                } else if self.current_count >= self.total_count {
                    presets::text_center("Waiting for the compression to complete...")
                } else {
                    presets::text_center(format!(
                        "Processing file {} of {}",
                        self.current_count, self.total_count,
                    ))
                };
                let max = self.total_count as f32;
                let current = self.current_count as f32;
                let bar = presets::progress_bar(current + max * 0.01, max * 1.03);
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::None, false),
                    status,
                    presets::button_nav(
                        "Cancel",
                        if let MergeStage::Cancelling(_) = self.stage {
                            Message::None
                        } else {
                            Message::Cancel
                        },
                        false,
                    ),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, bar.into(), brow.into()]).into()
            }
            MergeStage::Completed => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false),
                    presets::text_center("Merge completed"),
                    presets::button_nav("Repeat", Message::Repeat, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            MergeStage::Failed => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false),
                    presets::text_center_error("Merge failed"),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            MergeStage::Error => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false),
                    presets::text_center_error("Merge failed"),
                    presets::button_nav("Retry", Message::Repeat, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            MergeStage::Cancelled => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false),
                    presets::text_center_error("Merge cancelled"),
                    presets::button_nav("Retry", Message::Repeat, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
        }
    }
}
