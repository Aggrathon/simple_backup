#![cfg(feature = "gui")]

use iced::widget::Space;
use iced::{Command, Element, Length, Renderer, Subscription};
use regex::Regex;
use rfd::FileDialog;

use super::threads::ThreadWrapper;
use super::{paginated, presets, theme, Message};
use crate::backup::{BackupError, BackupReader};
use crate::files::FileInfo;

pub(crate) enum RestoreStage {
    Failed,
    Error(Box<BackupReader>),
    Viewing(Box<BackupReader>, Vec<(bool, String)>),
    Performing(ThreadWrapper<Result<FileInfo, BackupError>, BackupReader>),
    Cancelling(ThreadWrapper<Result<FileInfo, BackupError>, BackupReader>),
    Completed(Box<BackupReader>),
    Cancelled(Box<BackupReader>),
}

pub(crate) struct RestoreState {
    filter: String,
    filter_ok: bool,
    error: String,
    stage: RestoreStage,
    all: bool,
    flat: bool,
    pagination: paginated::State,
    extract: bool,
}

impl RestoreState {
    pub fn new(reader: BackupReader) -> Self {
        let mut state = Self {
            error: String::new(),
            stage: RestoreStage::Failed,
            all: true,
            filter: String::new(),
            filter_ok: true,
            flat: false,
            pagination: paginated::State::new(100, 0),
            extract: false,
        };
        state.view_list(reader);
        state
    }

    fn extract_reader(&mut self) -> Option<Box<BackupReader>> {
        self.stage = match std::mem::replace(&mut self.stage, RestoreStage::Failed) {
            RestoreStage::Error(br) => return Some(br),
            RestoreStage::Viewing(br, _) => return Some(br),
            RestoreStage::Completed(br) => return Some(br),
            RestoreStage::Cancelled(br) => return Some(br),
            x => x,
        };
        None
    }

    fn view_list(&mut self, mut reader: BackupReader) {
        match reader.get_meta() {
            Err(e) => {
                self.error.push_str("\nProblem with reading backup:");
                self.error.push_str(&e.to_string());
                self.view_error(reader);
            }
            Ok((_, list)) => {
                let list: Vec<_> = list.iter().map(|(_, s)| (true, String::from(s))).collect();
                self.pagination.set_total(list.len());
                self.all = true;
                self.stage = RestoreStage::Viewing(Box::new(reader), list);
            }
        }
    }

    fn try_view_list(&mut self) {
        if let Some(br) = self.extract_reader() {
            self.view_list(*br);
        }
    }

    fn view_error(&mut self, reader: BackupReader) {
        self.stage = RestoreStage::Error(Box::new(reader))
    }

    fn try_view_error(&mut self) {
        if let Some(br) = self.extract_reader() {
            self.view_error(*br);
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self.stage {
            RestoreStage::Performing(..) => {
                iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::Tick)
            }
            RestoreStage::Cancelling(..) => {
                iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick)
            }
            _ => Subscription::none(),
        }
    }

    fn filter_list(&mut self) {
        if let RestoreStage::Viewing(_, list) = &mut self.stage {
            let mut total = 0;
            let mut changed = false;
            if !self.filter.is_empty() {
                match Regex::new(&self.filter) {
                    Ok(regex) => {
                        self.filter_ok = true;
                        for i in 0..list.len() {
                            if regex.is_match(&list[i].1) {
                                if total != i {
                                    changed = true;
                                    list.swap(total, i);
                                }
                                total += 1;
                            }
                        }
                    }
                    Err(_) => {
                        self.filter_ok = false;
                        return;
                    }
                }
            } else {
                self.filter_ok = true;
                total = list.len();
            }
            if changed || self.pagination.get_total() != total {
                self.all = false;
                list[..total].sort_unstable_by(|(_, s1), (_, s2)| s1.cmp(s2));
                self.pagination.set_total(total);
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => match &mut self.stage {
                RestoreStage::Performing(wrapper) => {
                    for recv in wrapper {
                        match recv {
                            Ok(res) => match res {
                                Ok(_) => {
                                    self.pagination.index += 1;
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
                                    if let RestoreStage::Performing(wrapper) =
                                        std::mem::replace(&mut self.stage, RestoreStage::Failed)
                                    {
                                        match wrapper.join() {
                                            Ok(br) => {
                                                self.stage = RestoreStage::Completed(Box::new(br))
                                            }
                                            Err(_) => self.error.push_str(if self.extract {
                                                "\nFailure when finalising the extraction"
                                            } else {
                                                "\nFailure when finalising the restoration"
                                            }),
                                        }
                                    }
                                    break;
                                }
                            },
                        }
                    }
                }
                RestoreStage::Cancelling(..) => {
                    if let RestoreStage::Cancelling(wrapper) =
                        std::mem::replace(&mut self.stage, RestoreStage::Failed)
                    {
                        match wrapper.cancel() {
                            Ok(reader) => self.stage = RestoreStage::Cancelled(Box::new(reader)),
                            Err(_) => self.error.push_str(if self.extract {
                                "\nFailure when cancelling the extraction"
                            } else {
                                "\nFailure when cancelling the restoration"
                            }),
                        };
                    }
                }
                _ => {}
            },
            Message::Extract => {
                if let RestoreStage::Viewing(reader, _) = &self.stage {
                    if let Some(output) = FileDialog::new()
                        .set_directory(
                            reader
                                .config
                                .as_ref()
                                .expect("The config should be read at this point!")
                                .get_dir(),
                        )
                        .set_title("Select directory to extract to")
                        .pick_folder()
                    {
                        if let RestoreStage::Viewing(reader, list) =
                            std::mem::replace(&mut self.stage, RestoreStage::Failed)
                        {
                            self.extract = true;
                            self.stage = match ThreadWrapper::restore_files(
                                *reader,
                                list.into_iter()
                                    .filter_map(|(b, s)| if b { Some(s) } else { None })
                                    .collect(),
                                self.flat,
                                Some(output),
                                1000,
                            ) {
                                Ok(w) => RestoreStage::Performing(w),
                                Err((br, e)) => {
                                    self.error.push('\n');
                                    self.error.push_str(&e.to_string());
                                    RestoreStage::Error(Box::new(br))
                                }
                            }
                        }
                    }
                }
            }
            Message::Restore => {
                if let RestoreStage::Viewing(..) = &self.stage {
                    if let RestoreStage::Viewing(reader, list) =
                        std::mem::replace(&mut self.stage, RestoreStage::Failed)
                    {
                        self.pagination.set_total(list.len());
                        self.extract = false;
                        self.stage = match ThreadWrapper::restore_files(
                            *reader,
                            list.into_iter()
                                .filter_map(|(b, s)| if b { Some(s) } else { None })
                                .collect(),
                            false,
                            None,
                            1000,
                        ) {
                            Ok(w) => RestoreStage::Performing(w),
                            Err((br, e)) => {
                                self.error.push('\n');
                                self.error.push_str(&e.to_string());
                                RestoreStage::Error(Box::new(br))
                            }
                        };
                    }
                }
            }
            Message::Cancel => {
                if let RestoreStage::Performing(..) = &self.stage {
                    if let RestoreStage::Performing(wrapper) =
                        std::mem::replace(&mut self.stage, RestoreStage::Failed)
                    {
                        self.stage = RestoreStage::Cancelling(wrapper);
                    }
                }
            }
            Message::Toggle(i) => {
                if let RestoreStage::Viewing(_, list) = &mut self.stage {
                    if let Some((b, _)) = list.get_mut(i) {
                        *b = !*b;
                    }
                    self.all = false;
                }
            }
            Message::Flat(b) => self.flat = b,
            Message::Export => {
                if let RestoreStage::Viewing(reader, _) = &mut self.stage {
                    if let Some(file) = FileDialog::new()
                        .set_directory(reader.path.get_path())
                        .set_title("Save the list of files in the backup")
                        .set_file_name("files.txt")
                        .add_filter("Text file", &["txt"])
                        .add_filter("Csv file", &["csv"])
                        .save_file()
                    {
                        if let Err(e) = reader.export_list(file) {
                            self.error.push('\n');
                            self.error.push_str(&e.to_string());
                            self.pagination.set_total(0);
                            self.try_view_error();
                        }
                    }
                }
            }
            Message::ToggleAll => {
                if let RestoreStage::Viewing(_, list) = &mut self.stage {
                    self.all = !self.all;
                    list[..self.pagination.get_total()]
                        .iter_mut()
                        .for_each(|(b, _)| *b = self.all);
                }
            }
            Message::FilterEdit(_, s) => {
                self.filter = s;
                self.filter_list();
            }
            Message::GoTo(index) => {
                if let RestoreStage::Viewing(_, _) = &mut self.stage {
                    self.pagination.goto(index)
                }
            }
            Message::Repeat => {
                self.error.clear();
                self.try_view_list();
            }
            _ => eprintln!("Unexpected GUI message: {:?}", message),
        }
        Command::none()
    }

    pub fn view(&self) -> Element<Message, Renderer<theme::Theme>> {
        let mut scroll = presets::column_list();
        if !self.error.is_empty() {
            scroll = scroll.push(presets::text_error(&self.error[1..]))
        }
        match &self.stage {
            RestoreStage::Viewing(reader, list) => {
                scroll =
                    self.pagination
                        .push_to(scroll, list.iter().enumerate(), |(i, (sel, file))| {
                            presets::checkbox(*sel, file, move |_| Message::Toggle(i))
                                .width(Length::Fill)
                                .into()
                        });
                let trow = presets::row_list2(vec![
                    presets::space_inner().into(),
                    presets::checkbox(self.all, "", |_| Message::ToggleAll).into(),
                    presets::space_large().into(),
                    presets::regex_field(&self.filter, "Search", self.filter_ok, |s| {
                        Message::FilterEdit(0, s)
                    })
                    .width(Length::Fill)
                    .on_submit(Message::FilterAdd)
                    .into(),
                ]);
                let status = match reader
                    .config
                    .as_ref()
                    .expect("The config should already be read at this point!")
                    .time
                {
                    Some(t) => format!(
                        "{} files from {}",
                        list.len(),
                        t.format("%Y-%m-%d %H:%M:%S")
                    ),
                    None => format!("{} files", list.len(),),
                };
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    presets::text_center(status).into(),
                    presets::button("Export list", Message::Export).into(),
                    presets::space_large().into(),
                    presets::toggler(self.flat, "Flat", Message::Flat).into(),
                    presets::space_large().into(),
                    presets::button("Extract", Message::Extract).into(),
                    presets::button("Restore", Message::Restore).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![trow.into(), scroll.into(), brow.into()]).into()
            }
            RestoreStage::Error(_) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    if self.extract {
                        presets::text_center_error("Extraction failed").into()
                    } else {
                        presets::text_center_error("Restoration failed").into()
                    },
                    presets::button_nav("Retry", Message::Repeat, true).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll.into(), brow.into()]).into()
            }
            RestoreStage::Performing(_) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Cancel", Message::Cancel, false).into(),
                    presets::text_center(if self.extract {
                        format!(
                            "Extracting files: {} / {}",
                            self.pagination.index,
                            self.pagination.get_total(),
                        )
                    } else {
                        format!(
                            "Restoring files: {} / {}",
                            self.pagination.index,
                            self.pagination.get_total(),
                        )
                    })
                    .into(),
                ]);
                let pb = presets::progress_bar2(self.pagination.index, self.pagination.get_total());
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll.into(), pb.into(), brow.into()]).into()
            }
            RestoreStage::Cancelling(_) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Cancel", Message::None, false).into(),
                    if self.extract {
                        presets::text_center_error("Cancelling the extraction").into()
                    } else {
                        presets::text_center_error("Cancelling the restoration").into()
                    },
                ]);
                let pb = presets::progress_bar2(self.pagination.index, self.pagination.get_total());
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll.into(), pb.into(), brow.into()]).into()
            }
            RestoreStage::Completed(_) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    if self.extract {
                        presets::text_center("Extraction complete").into()
                    } else {
                        presets::text_center("Restoration complete").into()
                    },
                    presets::button_nav("Repeat", Message::Repeat, true).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll.into(), brow.into()]).into()
            }
            RestoreStage::Cancelled(_) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    if self.extract {
                        presets::text_center("Extraction cancelled").into()
                    } else {
                        presets::text_center("Restoration cancelled").into()
                    },
                    presets::button_nav("Retry", Message::Repeat, true).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll.into(), brow.into()]).into()
            }
            RestoreStage::Failed => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    presets::text_center(if self.extract {
                        "Extraction failed"
                    } else {
                        "Restoration failed"
                    })
                    .into(),
                    Space::with_width(Length::Fill).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_root(vec![scroll.into(), brow.into()]).into()
            }
        }
    }
}
