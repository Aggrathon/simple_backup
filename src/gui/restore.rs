#![cfg(feature = "gui")]

use iced::pure::Element;
use iced::{Command, Length, Space};
use regex::Regex;
use rfd::FileDialog;

use super::{paginated, presets, Message};
use crate::backup::BackupReader;

pub(crate) enum RestoreStage<'a> {
    Error,
    View(BackupReader<'a>, Vec<(bool, String)>),
    // Extract,
}

pub(crate) struct RestoreState<'a> {
    filter: String,
    filter_ok: bool,
    error: String,
    stage: RestoreStage<'a>,
    all: bool,
    flat: bool,
    pagination: paginated::State,
}

impl<'a> RestoreState<'a> {
    pub fn new(mut reader: BackupReader<'a>) -> Self {
        let mut state = Self {
            error: String::new(),
            stage: RestoreStage::Error,
            all: false,
            filter: String::new(),
            filter_ok: true,
            flat: false,
            pagination: paginated::State::new(100, 0),
        };
        if let Err(e) = reader.read_all() {
            state.error.push('\n');
            state.error.push_str(&e.to_string());
            return state;
        }
        let list: Vec<(bool, String)> = reader
            .get_list()
            .expect("The list should already be extracted")
            .split('\n')
            .map(|s| (false, String::from(s)))
            .collect();
        state.pagination.set_total(list.len());
        state.stage = RestoreStage::View(reader, list);
        state
    }

    fn filter_list(&mut self) {
        if let RestoreStage::View(_, list) = &mut self.stage {
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
            Message::Toggle(i) => {
                if let RestoreStage::View(_, list) = &mut self.stage {
                    if let Some((b, _)) = list.get_mut(i) {
                        *b = !*b;
                    }
                    self.all = false;
                }
            }
            Message::Flat(b) => self.flat = b,
            Message::Extract => todo!(),
            Message::Restore => todo!(),
            Message::Export => {
                if let RestoreStage::View(reader, _) = &mut self.stage {
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
                if let RestoreStage::View(_, list) = &mut self.stage {
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
                if let RestoreStage::View(_, _) = &mut self.stage {
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
            RestoreStage::Error => Space::with_height(Length::Shrink).into(),
            RestoreStage::View(_, list) => {
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
        };
        let mut brow = presets::row_bar(vec![
            presets::button_nav("Back", Message::MainView, false).into(),
            Space::with_width(Length::Fill).into(),
        ]);
        if let RestoreStage::View(reader, list) = &self.stage {
            brow = brow
                .push(presets::text(&match reader
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
                .push(presets::button_color("Restore", Message::Restore));
        }
        let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
        presets::column_main(vec![trow, scroll.into(), brow.into()]).into()
    }
}
