#![cfg(feature = "gui")]

use iced::pure::Element;
use iced::{Command, Length, Space, Subscription};
use regex::Regex;
use rfd::FileDialog;

use super::threads::ThreadWrapper;
use super::{paginated, presets, Message};
use crate::backup::{BackupError, BackupReader};
use crate::files::FileInfo;

pub(crate) enum RestoreStage {
    Error,
    Viewing(BackupReader, Vec<(bool, String)>),
    Performing(
        ThreadWrapper<Result<FileInfo, BackupError>, BackupReader>,
        bool,
    ),
    Cancelling(
        ThreadWrapper<Result<FileInfo, BackupError>, BackupReader>,
        bool,
    ),
}

pub(crate) struct RestoreState {
    filter: String,
    filter_ok: bool,
    error: String,
    stage: RestoreStage,
    all: bool,
    flat: bool,
    pagination: paginated::State,
}

impl RestoreState {
    pub fn new(reader: BackupReader) -> Self {
        let mut state = Self {
            error: String::new(),
            stage: RestoreStage::Error,
            all: true,
            filter: String::new(),
            filter_ok: true,
            flat: false,
            pagination: paginated::State::new(100, 0),
        };
        state.view_list(reader);
        state
    }

    fn view_list(&mut self, mut reader: BackupReader) {
        match reader.get_meta() {
            Err(e) => {
                self.error.push('\n');
                self.error.push_str(&e.to_string());
                self.stage = RestoreStage::Error;
            }
            Ok((_, list)) => {
                let list: Vec<(bool, String)> =
                    list.split('\n').map(|s| (true, String::from(s))).collect();
                self.pagination.set_total(list.len());
                self.all = true;
                self.stage = RestoreStage::Viewing(reader, list);
            }
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
            Message::Tick => match &self.stage {
                RestoreStage::Performing(wrapper, _) => {
                    for _ in 0..1000 {
                        match wrapper.try_recv() {
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
                                    if let RestoreStage::Performing(wrapper, restore) =
                                        std::mem::replace(&mut self.stage, RestoreStage::Error)
                                    {
                                        match wrapper.join() {
                                            Ok(br) => self.view_list(br),
                                            Err(_) => self.error.push_str(if restore {
                                                "\nFailure when finalising the restoration"
                                            } else {
                                                "\nFailure when finalising the extraction"
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
                    if let RestoreStage::Cancelling(wrapper, restore) =
                        std::mem::replace(&mut self.stage, RestoreStage::Error)
                    {
                        match wrapper.cancel() {
                            Ok(reader) => self.view_list(reader),
                            Err(_) => self.error.push_str(if restore {
                                "\nFailure when cancelling the restoration"
                            } else {
                                "\nFailure when cancelling the extraction"
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
                            std::mem::replace(&mut self.stage, RestoreStage::Error)
                        {
                            self.stage = match ThreadWrapper::restore_files(
                                reader,
                                list.into_iter()
                                    .filter_map(|(b, s)| if b { Some(s) } else { None })
                                    .collect(),
                                self.flat,
                                Some(output),
                            ) {
                                Ok(w) => RestoreStage::Performing(w, false),
                                Err(e) => {
                                    self.error.push('\n');
                                    self.error.push_str(&e.to_string());
                                    RestoreStage::Error
                                }
                            }
                        }
                    }
                }
            }
            Message::Restore => {
                if let RestoreStage::Viewing(..) = &self.stage {
                    if let RestoreStage::Viewing(reader, list) =
                        std::mem::replace(&mut self.stage, RestoreStage::Error)
                    {
                        self.pagination.set_total(list.len());
                        self.stage = match ThreadWrapper::restore_files(
                            reader,
                            list.into_iter()
                                .filter_map(|(b, s)| if b { Some(s) } else { None })
                                .collect(),
                            false,
                            None,
                        ) {
                            Ok(w) => RestoreStage::Performing(w, true),
                            Err(e) => {
                                self.error.push('\n');
                                self.error.push_str(&e.to_string());
                                RestoreStage::Error
                            }
                        };
                    }
                }
            }
            Message::Cancel => {
                if let RestoreStage::Performing(..) = &self.stage {
                    if let RestoreStage::Performing(wrapper, restore) =
                        std::mem::replace(&mut self.stage, RestoreStage::Error)
                    {
                        self.stage = RestoreStage::Cancelling(wrapper, restore);
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
                        .set_directory(&reader.path)
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
                            self.stage = RestoreStage::Error;
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
            _ => {}
        }
        Command::none()
    }

    pub fn view(&self) -> Element<Message> {
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
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    presets::text_center(&match reader
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
                    })
                    .into(),
                    presets::button_color("Export list", Message::Export).into(),
                    presets::toggler(self.flat, "Flat", Message::Flat).into(),
                    presets::button_color("Extract", Message::Extract).into(),
                    presets::button_color("Restore", Message::Restore).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_main(vec![trow.into(), scroll.into(), brow.into()]).into()
            }
            RestoreStage::Error => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    Space::with_width(Length::Fill).into(),
                ]);
                let scroll = presets::scroll_border(scroll.into());
                presets::column_main(vec![scroll.into(), brow.into()]).into()
            }
            RestoreStage::Performing(_, restore) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Cancel", Message::Cancel, false).into(),
                    presets::text_center(if *restore {
                        format!(
                            "Restoring files: {} / {}",
                            self.pagination.index,
                            self.pagination.get_total(),
                        )
                    } else {
                        format!(
                            "Extracting files: {} / {}",
                            self.pagination.index,
                            self.pagination.get_total(),
                        )
                    })
                    .into(),
                ]);
                let pb = presets::progress_bar2(self.pagination.index, self.pagination.get_total());
                let scroll = presets::scroll_border(scroll.into());
                presets::column_main(vec![scroll.into(), pb.into(), brow.into()]).into()
            }
            RestoreStage::Cancelling(_, restore) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Cancel", Message::None, false).into(),
                    presets::text_center_error(if *restore {
                        "Cancelling the restoration"
                    } else {
                        "Cancelling the extraction"
                    })
                    .into(),
                ]);
                let pb = presets::progress_bar2(self.pagination.index, self.pagination.get_total());
                let scroll = presets::scroll_border(scroll.into());
                presets::column_main(vec![scroll.into(), pb.into(), brow.into()]).into()
            }
        }
    }
}
