#![cfg(feature = "gui")]

use std::path::PathBuf;

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
    Performing(ThreadWrapper<Result<FileInfo, BackupError>, BackupReader>),
    Cancelling(ThreadWrapper<Result<FileInfo, BackupError>, BackupReader>),
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
    pub fn new(mut reader: BackupReader) -> Self {
        let mut state = Self {
            error: String::new(),
            stage: RestoreStage::Error,
            all: true,
            filter: String::new(),
            filter_ok: true,
            flat: false,
            pagination: paginated::State::new(100, 0),
        };
        if let Err(e) = reader.get_meta() {
            state.error.push('\n');
            state.error.push_str(&e.to_string());
            return state;
        }
        let list: Vec<(bool, String)> = reader
            .get_list()
            .expect("The list should already be extracted")
            .split('\n')
            .map(|s| (true, String::from(s)))
            .collect();
        state.pagination.set_total(list.len());
        state.stage = RestoreStage::Viewing(reader, list);
        state
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self.stage {
            RestoreStage::Performing(_) => {
                iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::Tick)
            }
            RestoreStage::Cancelling(_) => {
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
                list[..total].sort_unstable_by(|(_, s1), (_, s2)| s1.cmp(s2));
                self.pagination.set_total(total);
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        //TODO Restore func
        match message {
            Message::Tick => {
                // Handle restoring
                match &self.stage {
                    RestoreStage::Performing(_) => todo!(),
                    RestoreStage::Cancelling(_) => todo!(),
                    _ => {}
                }
            }
            Message::Extract => {
                if let RestoreStage::Viewing(..) = &self.stage {
                    // TODO get the output file
                    let output = PathBuf::new();
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
                            Ok(w) => RestoreStage::Performing(w),
                            Err(e) => {
                                self.error.push('\n');
                                self.error.push_str(&e.to_string());
                                RestoreStage::Error
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
                        self.stage = match ThreadWrapper::restore_files(
                            reader,
                            list.into_iter()
                                .filter_map(|(b, s)| if b { Some(s) } else { None })
                                .collect(),
                            false,
                            None,
                        ) {
                            Ok(w) => RestoreStage::Performing(w),
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
                if let RestoreStage::Performing(_) = &self.stage {
                    if let RestoreStage::Performing(wrapper) =
                        std::mem::replace(&mut self.stage, RestoreStage::Error)
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
        let trow = match &self.stage {
            RestoreStage::Viewing(_, list) => {
                scroll =
                    self.pagination
                        .push_to(scroll, list.iter().enumerate(), |(i, (sel, file))| {
                            presets::checkbox(*sel, file, move |_| Message::Toggle(i))
                                .width(Length::Fill)
                                .into()
                        });
                presets::row_list2(vec![
                    presets::space_inner().into(),
                    presets::checkbox(self.all, "", |_| Message::ToggleAll).into(),
                    presets::space_large().into(),
                    presets::regex_field(&self.filter, "Search", self.filter_ok, |s| {
                        Message::FilterEdit(0, s)
                    })
                    .width(Length::Fill)
                    .on_submit(Message::FilterAdd)
                    .into(),
                ])
                .into()
            }
            _ => Space::with_height(Length::Shrink).into(),
        };
        let brow = match &self.stage {
            RestoreStage::Viewing(reader, list) => {
                let brow = presets::row_bar(vec![
                    presets::button_nav("Back", Message::MainView, false).into(),
                    Space::with_width(Length::Fill).into(),
                ]);
                brow.push(presets::text(&match reader
                    .config
                    .as_ref()
                    .expect("The config should already be read")
                    .time
                {
                    Some(t) => format!(
                        "{} files from {}",
                        list.len(),
                        t.format("%Y-%m-%d %H:%M:%S")
                    ),
                    None => format!("{} files", list.len(),),
                }))
                .push(Space::with_width(Length::Fill))
                .push(presets::button_color("Export list", Message::Export))
                .push(presets::toggler(self.flat, "Flat", Message::Flat))
                .push(presets::button_color("Extract", Message::Extract))
                .push(presets::button_color("Restore", Message::Restore))
            }
            RestoreStage::Error => presets::row_bar(vec![
                presets::button_nav("Back", Message::MainView, false).into(),
                Space::with_width(Length::Fill).into(),
            ]),
            RestoreStage::Performing(_) => presets::row_bar(vec![
                presets::button_nav("Cancel", Message::Cancel, false).into(),
                Space::with_width(Length::Fill).into(),
            ]),
            RestoreStage::Cancelling(_) => {
                presets::row_bar(vec![Space::with_width(Length::Fill).into()])
            }
        };
        let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
        presets::column_main(vec![trow, scroll.into(), brow.into()]).into()
    }
}
