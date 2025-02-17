#![cfg(feature = "gui")]

use iced::alignment::Horizontal;
use iced::{Element, Length, Subscription};
use rfd::FileDialog;

use super::threads::ThreadWrapper;
use super::{paginated, presets, Message};
use crate::backup::{BackupError, BackupWriter};
use crate::config::Config;
use crate::files::FileInfo;
use crate::utils::format_size;

#[derive(PartialEq, Eq)]
enum ListSort {
    Name,
    Size,
    Time,
}

#[allow(clippy::large_enum_variant)]
enum BackupStage {
    Failed,
    Scanning(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Viewing(BackupWriter),
    Performing(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Cancelling(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Completed,
    Cancelled,
}

pub(crate) struct BackupState {
    pub config: Config,
    list_sort: ListSort,
    error: String,
    total_count: usize,
    total_size: u64,
    current_count: usize,
    current_size: u64,
    stage: BackupStage,
    pagination: paginated::State,
}

impl BackupState {
    pub fn new(config: Config) -> Self {
        let crawler = ThreadWrapper::crawl_for_files(config.clone(), 1000);
        Self {
            config,
            list_sort: ListSort::Name,
            error: String::new(),
            total_count: 0,
            total_size: 0,
            current_count: 0,
            current_size: 0,
            stage: BackupStage::Scanning(crawler),
            pagination: paginated::State::new(100, 0),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tick => match &mut self.stage {
                BackupStage::Scanning(crawler) => {
                    for recv in crawler {
                        match recv {
                            Ok(res) => match res {
                                Ok(fi) => {
                                    self.total_count += 1;
                                    self.total_size += fi.size;
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
                                    if let BackupStage::Scanning(crawler) =
                                        std::mem::replace(&mut self.stage, BackupStage::Failed)
                                    {
                                        match crawler.join() {
                                            Ok(mut bw) => {
                                                if self.config.incremental && bw.prev_time.is_some()
                                                {
                                                    self.total_count = 0;
                                                    self.total_size = 0;
                                                    if let Err(e) = bw.foreach_file(false, |res| {
                                                        #[allow(unused_must_use)]
                                                        if let Ok(fi) = res {
                                                            self.total_count += 1;
                                                            self.total_size += fi.size;
                                                        }
                                                        Ok(())
                                                    }) {
                                                        self.error.push('\n');
                                                        self.error.push_str(&e.to_string());
                                                    };
                                                }
                                                self.pagination.set_total(self.total_count);
                                                self.stage = BackupStage::Viewing(bw);
                                            }
                                            Err(_) => self.error.push_str(
                                                "\nFailure when finalising the list of files",
                                            ),
                                        }
                                    }
                                    break;
                                }
                            },
                        }
                    }
                }
                BackupStage::Performing(wrapper) => {
                    for recv in wrapper {
                        match recv {
                            Ok(res) => match res {
                                Ok(fi) => {
                                    self.current_count += 1;
                                    self.current_size += fi.size;
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
                                    if let BackupStage::Performing(wrapper) =
                                        std::mem::replace(&mut self.stage, BackupStage::Failed)
                                    {
                                        match wrapper.join() {
                                            Ok(_) => {
                                                self.current_count = 0;
                                                self.stage = BackupStage::Completed
                                            }
                                            Err(_) => self
                                                .error
                                                .push_str("\nFailure when finalising the backup"),
                                        }
                                    }
                                    break;
                                }
                            },
                        }
                    }
                }
                BackupStage::Cancelling(wrapper) => {
                    if wrapper.try_cancel() {
                        if let BackupStage::Cancelling(wrapper) =
                            std::mem::replace(&mut self.stage, BackupStage::Failed)
                        {
                            match wrapper.cancel() {
                                Ok(writer) => {
                                    if let Err(e) = writer.delete_file() {
                                        self.error.push('\n');
                                        self.error.push_str(&e.to_string());
                                    }
                                    self.current_count = 0;
                                    self.stage = BackupStage::Cancelled
                                }
                                Err(_) => {
                                    self.error.push_str("\nFailure when cancelling the backup")
                                }
                            };
                        }
                    }
                }
                _ => {}
            },
            Message::SortName => {
                self.list_sort = ListSort::Name;
                if let BackupStage::Viewing(writer) = &mut self.stage {
                    writer.list.as_mut().unwrap().sort_unstable();
                }
            }
            Message::SortSize => {
                self.list_sort = ListSort::Size;
                if let BackupStage::Viewing(writer) = &mut self.stage {
                    writer
                        .list
                        .as_mut()
                        .unwrap()
                        .sort_unstable_by(|a, b| b.size.cmp(&a.size));
                }
            }
            Message::SortTime => {
                self.list_sort = ListSort::Time;
                if let BackupStage::Viewing(writer) = &mut self.stage {
                    writer
                        .list
                        .as_mut()
                        .unwrap()
                        .sort_unstable_by(|a, b| b.time.unwrap().cmp(&a.time.unwrap()));
                }
            }
            Message::Backup => {
                if let BackupStage::Viewing(_) = &self.stage {
                    self.list_sort = ListSort::Name;
                    if let BackupStage::Viewing(mut writer) =
                        std::mem::replace(&mut self.stage, BackupStage::Failed)
                    {
                        writer.list.as_mut().unwrap().sort_unstable();
                        self.stage =
                            BackupStage::Performing(ThreadWrapper::backup_files(writer, 1000));
                        self.current_count = 0;
                        self.current_size = 0;
                    }
                }
            }
            Message::Cancel => {
                if let BackupStage::Performing(_) = &self.stage {
                    if let BackupStage::Performing(wrapper) =
                        std::mem::replace(&mut self.stage, BackupStage::Failed)
                    {
                        self.stage = BackupStage::Cancelling(wrapper);
                    }
                }
            }
            Message::Export => {
                if let BackupStage::Viewing(writer) = &mut self.stage {
                    if let Some(file) = FileDialog::new()
                        .set_directory(self.config.get_output(true))
                        .set_title("Save list of files to backup")
                        .set_file_name("files.txt")
                        .add_filter("Text file", &["txt"])
                        .add_filter("Csv file", &["csv"])
                        .save_file()
                    {
                        if let Err(e) = writer.export_list(file, false) {
                            self.error.push('\n');
                            self.error.push_str(&e.to_string());
                        }
                    }
                }
            }
            Message::GoTo(index) => {
                if let BackupStage::Viewing(_) = self.stage {
                    self.pagination.goto(index)
                }
            }
            Message::Repeat => *self = BackupState::new(std::mem::take(&mut self.config)),
            _ => eprintln!("Unexpected GUI message: {:?}", message),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self.stage {
            BackupStage::Scanning(_) => {
                iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::Tick)
            }
            BackupStage::Performing(_) => {
                iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::Tick)
            }
            BackupStage::Cancelling(_) => {
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
            BackupStage::Scanning(_) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Edit", Message::EditConfig, false),
                    presets::text_center(format!(
                        "Scanning for files to backup: {} with total size {}\n",
                        self.total_count,
                        format_size(self.total_size)
                    )),
                    presets::button_nav("Backup", Message::None, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            BackupStage::Viewing(writer) => {
                let trow = presets::row_list2(vec![
                    presets::button_group(
                        "Name",
                        Message::SortName,
                        self.list_sort == ListSort::Name,
                    )
                    .width(Length::Fill)
                    .into(),
                    presets::button_group(
                        "Size",
                        Message::SortSize,
                        self.list_sort == ListSort::Size,
                    )
                    .width(Length::Fixed(102.0))
                    .into(),
                    presets::button_group(
                        "Time",
                        Message::SortTime,
                        self.list_sort == ListSort::Time,
                    )
                    .width(Length::Fixed(182.0))
                    .into(),
                ]);
                scroll = self.pagination.push_to(
                    scroll,
                    writer
                        .list
                        .as_ref()
                        .expect("The files should already be crawled at this point!")
                        .iter()
                        .filter_map(|(b, fi)| if *b { Some(fi) } else { None }),
                    |f| {
                        presets::row_list2(vec![
                            presets::text(f.copy_string()).width(Length::Fill).into(),
                            presets::text(format_size(f.size))
                                .width(Length::Fixed(102.0))
                                .align_x(Horizontal::Right)
                                .into(),
                            presets::text(f.time.unwrap().format("%Y-%m-%d %H:%M:%S").to_string())
                                .width(Length::Fixed(182.0))
                                .align_x(Horizontal::Right)
                                .into(),
                            presets::space_scroll(),
                        ])
                        .into()
                    },
                );
                let diff = writer.list.as_ref().unwrap().len() - self.total_count;
                let status = if diff > 0 {
                    if let Some(time) = writer.prev_time {
                        format!(
                            "{} files with total size {} ({} files have not changed since {})",
                            self.total_count,
                            format_size(self.total_size),
                            diff,
                            time
                        )
                    } else {
                        format!(
                            "{} files with total size {} ({} files have not changed)",
                            self.total_count,
                            format_size(self.total_size),
                            diff
                        )
                    }
                } else {
                    format!(
                        "{} files with total size {}",
                        self.total_count,
                        format_size(self.total_size)
                    )
                };
                let brow = presets::row_bar(vec![
                    presets::button_nav("Edit", Message::EditConfig, false),
                    presets::text_center(status),
                    presets::button("Export list", Message::Export),
                    presets::button_nav("Backup", Message::Backup, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![trow.into(), scroll, brow.into()]).into()
            }
            BackupStage::Performing(_) | BackupStage::Cancelling(_) => {
                let status = if let BackupStage::Cancelling(_) = self.stage {
                    presets::text_center_error("Cancelling the backup...")
                } else if self.current_count >= self.total_count {
                    presets::text_center("Waiting for the compression to complete...")
                } else {
                    presets::text_center(format!(
                        "Backing up file {} of {} ({} of {})",
                        self.current_count,
                        self.total_count,
                        format_size(self.current_size),
                        format_size(self.total_size)
                    ))
                };
                let max = (self.total_size / 1024 + self.total_count as u64) as f32;
                let current = (self.current_size / 1024 + self.current_count as u64) as f32;
                let bar = presets::progress_bar(current + max * 0.01, max * 1.03);
                let brow = presets::row_bar(vec![
                    presets::button_nav("Edit", Message::None, false),
                    status,
                    presets::button_nav(
                        "Cancel",
                        if let BackupStage::Cancelling(_) = self.stage {
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
            BackupStage::Failed => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Edit", Message::EditConfig, false),
                    presets::text_center_error("Backup failed"),
                    presets::button_nav("Retry", Message::Repeat, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            BackupStage::Completed => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Edit", Message::EditConfig, false),
                    presets::text_center("Backup completed"),
                    presets::button_nav("Repeat", Message::Repeat, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
            BackupStage::Cancelled => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Edit", Message::EditConfig, false),
                    presets::text_center_error("Backup cancelled"),
                    presets::button_nav("Retry", Message::Repeat, true),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll, brow.into()]).into()
            }
        }
    }
}
