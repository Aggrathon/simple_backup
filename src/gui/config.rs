#![cfg(feature = "gui")]

use std::path::PathBuf;

use iced::widget::pane_grid;
use iced::{Element, Length};
use regex::Regex;
use rfd::{FileDialog, MessageDialog};

use super::{presets, Message};
use crate::backup::{CONFIG_DEFAULT_NAME, CONFIG_FILE_EXTENSION};
use crate::config::Config;
use crate::files::{FileCrawler, FileInfo};
use crate::utils::{default_dir, home_dir};

pub(crate) struct ConfigState {
    pub config: Config,
    panes: pane_grid::State<Pane>,
    thread_alt: Vec<u32>,
    compression_alt: Vec<i32>,
    files: pane_grid::Pane,
    includes: pane_grid::Pane,
    excludes: pane_grid::Pane,
    filters: pane_grid::Pane,
    current_dir: FileInfo,
}

impl ConfigState {
    pub fn new(open_home: bool, default_ignores: bool) -> Self {
        let (mut panes, files) = pane_grid::State::new(Pane::new(ConfigPane::Files));
        let (includes, _) = panes
            .split(
                pane_grid::Axis::Vertical,
                files,
                Pane::new(ConfigPane::Includes),
            )
            .unwrap();
        let (excludes, _) = panes
            .split(
                pane_grid::Axis::Horizontal,
                includes,
                Pane::new(ConfigPane::Excludes),
            )
            .unwrap();
        let (filters, _) = panes
            .split(
                pane_grid::Axis::Horizontal,
                excludes,
                Pane::new(ConfigPane::Filters),
            )
            .unwrap();
        let mut config = Config::new();
        if default_ignores {
            config.add_default_ignores();
        }
        let mut state = Self {
            config,
            panes,
            thread_alt: (1..=num_cpus::get() as u32).collect(),
            compression_alt: (1..=22).collect(),
            files,
            includes,
            excludes,
            filters,
            current_dir: FileInfo::from(if open_home { home_dir() } else { default_dir() }),
        };
        if open_home | default_ignores {
            state.refresh_filters();
            state.refresh_files();
        }
        state
    }

    pub fn from(mut config: Config) -> Self {
        config.sort();
        let mut state = Self::new(false, false);
        state.current_dir = FileInfo::from(config.get_dir());
        state.config = config;
        state.refresh_includes();
        state.refresh_excludes();
        state.refresh_filters();
        state.refresh_files();
        state
    }

    pub fn view(&self) -> Element<Message> {
        let pane_grid = presets::pane_grid(&self.panes, |_, pane, _| pane.content());
        let bar = presets::row_bar(vec![
            presets::button_nav("Back", Message::MainView, false),
            presets::space_hfill(),
            presets::text("Compression:").into(),
            presets::pick_list(
                &self.compression_alt,
                Some(self.config.quality),
                Message::CompressionQuality,
            ),
            presets::space_large(),
            presets::text("Threads:").into(),
            presets::pick_list(
                &self.thread_alt,
                Some(self.config.threads),
                Message::ThreadCount,
            ),
            presets::space_large(),
            presets::toggler(
                self.config.incremental,
                "Incremental backups:",
                Message::Incremental,
            ),
            presets::space_hfill(),
            presets::button_nav("Save", Message::Save, true),
            presets::button_nav("Backup", Message::BackupView, true),
        ]);
        presets::column_root(vec![pane_grid.into(), bar.into()]).into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio)
            }
            Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.drop(pane, target)
            }
            Message::PaneDragged(_) => {}
            Message::Incremental(t) => self.config.incremental = t,
            Message::ThreadCount(text) => self.config.set_threads(text),
            Message::CompressionQuality(text) => self.config.set_quality(text),
            Message::IncludeAdd(i) => {
                let pane = self.panes.get_mut(self.files).unwrap();
                if let Some(li) = pane.items.get_mut(i) {
                    let s = std::mem::take(&mut li.text);
                    if let Ok(i) = self.config.exclude.binary_search(&s) {
                        self.config.exclude.remove(i);
                        self.refresh_excludes();
                    }
                    self.config.include.push(s);
                    self.config.include.sort_unstable();
                    self.refresh_includes();
                    self.refresh_files();
                }
            }
            Message::IncludeRemove(i) => {
                if i < self.config.include.len() {
                    self.config.include.remove(i);
                    self.refresh_includes();
                    self.refresh_files();
                }
            }
            Message::IncludeOpen(i) => {
                if let Some(s) = self.config.include.get(i) {
                    let p = PathBuf::from(s);
                    if let Ok(m) = p.metadata() {
                        if m.is_dir() {
                            self.open_dir(p);
                        } else if let Some(p) = p.parent() {
                            self.open_dir(p);
                        }
                    }
                }
            }
            Message::ExcludeAdd(i) => {
                let pane = self.panes.get_mut(self.files).unwrap();
                if let Some(li) = pane.items.get_mut(i) {
                    let s = std::mem::take(&mut li.text);
                    if let Ok(i) = self.config.include.binary_search(&s) {
                        self.config.include.remove(i);
                        self.refresh_includes();
                    }
                    self.config.exclude.push(s);
                    self.config.exclude.sort_unstable();
                    self.refresh_excludes();
                    self.refresh_files();
                }
            }
            Message::ExcludeRemove(i) => {
                if i < self.config.exclude.len() {
                    self.config.exclude.remove(i);
                    self.refresh_excludes();
                    self.refresh_files();
                }
            }
            Message::ExcludeOpen(i) => {
                if let Some(s) = self.config.exclude.get(i) {
                    let p = PathBuf::from(s);
                    if let Ok(m) = p.metadata() {
                        if m.is_dir() {
                            self.open_dir(p);
                        } else if let Some(p) = p.parent() {
                            self.open_dir(p);
                        }
                    }
                }
            }
            Message::FilterAdd => {
                self.config.regex.push(String::new());
                self.refresh_filters();
            }
            Message::FilterRemove(i) => {
                if i < self.config.regex.len() {
                    self.config.regex.remove(i);
                    self.refresh_filters();
                    self.refresh_files();
                }
            }
            Message::FilterEdit(i, s) => {
                let pane = self.panes.get_mut(self.filters).unwrap();
                let mut refresh = false;
                if let Some(item) = pane.items.get_mut(i) {
                    if !item.text.eq(&s) {
                        if Regex::new(&s).is_ok() {
                            item.status = true;
                            refresh = true;
                            self.config.regex[i].replace_range(.., &s);
                        } else {
                            refresh = item.status;
                            item.status = false;
                        }
                        item.text = s;
                    }
                }
                if refresh {
                    self.refresh_files();
                }
            }
            Message::FolderOpen(i) => {
                let pane = self.panes.get_mut(self.files).unwrap();
                if let Some(li) = pane.items.get_mut(i) {
                    let dir: FileInfo = std::mem::take(&mut li.text).into();
                    self.open_dir(dir);
                }
            }
            Message::FolderDialog => {
                if let Some(folder) = FileDialog::new()
                    .set_directory(self.current_dir.get_path())
                    .set_title("Open Directory")
                    .pick_folder()
                {
                    self.open_dir(folder);
                }
            }
            Message::FolderUp => {
                if let Some(dir) = self.current_dir.get_path().parent() {
                    let dir: FileInfo = dir.into();
                    self.open_dir(dir);
                }
            }
            Message::Save => {
                if let Some(file) = FileDialog::new()
                    .set_directory(self.config.get_output(true))
                    .set_title("Save config file")
                    .set_file_name(CONFIG_DEFAULT_NAME)
                    .add_filter("Config file", &[&CONFIG_FILE_EXTENSION[1..]])
                    .save_file()
                {
                    match self.config.write_yaml(file, false) {
                        Ok(_) => {}
                        Err(e) => {
                            MessageDialog::new()
                                .set_description(e.to_string())
                                .set_level(rfd::MessageLevel::Error)
                                .set_buttons(rfd::MessageButtons::Ok)
                                .set_title("Problem saving config")
                                .show();
                        }
                    };
                }
            }
            _ => eprintln!("Unexpected GUI message: {:?}", message),
        }
    }

    fn open_dir<P: Into<FileInfo>>(&mut self, folder: P) {
        self.current_dir = folder.into();
        self.refresh_files();
    }

    fn refresh_files(&mut self) {
        let pane = self.panes.get_mut(self.files).unwrap();
        pane.items.clear();
        match FileCrawler::new(
            &self.config.include,
            &self.config.exclude,
            &self.config.regex,
            self.config.local,
        ) {
            Ok(fc) => {
                let parent = fc.check_path(&mut self.current_dir, None);
                pane.items.push(ListItem::new(
                    ListState::ParentFolder(self.current_dir.get_path().parent().is_some()),
                    self.current_dir.get_string().to_string(),
                    0,
                    parent,
                ));
                match self.current_dir.get_path().read_dir() {
                    Ok(rd) => {
                        for (i, de) in rd.into_iter().enumerate() {
                            match de {
                                Ok(de) => match de.metadata() {
                                    Ok(md) => {
                                        let dir = md.is_dir();
                                        let mut fi = FileInfo::from(&de);
                                        let inc = fc.check_path(&mut fi, Some(parent));
                                        pane.items.push(ListItem::file(
                                            fi.move_string(),
                                            inc,
                                            dir,
                                            i + 1,
                                        ));
                                    }
                                    Err(e) => pane.items.push(ListItem::error(format!("{}", e))),
                                },
                                Err(e) => pane.items.push(ListItem::error(format!("{}", e))),
                            }
                        }
                    }
                    Err(e) => pane.items.push(ListItem::error(format!("{}", e))),
                }
            }
            Err(e) => pane.items.push(ListItem::error(format!("{}", e))),
        };
    }

    fn refresh_includes(&mut self) {
        let pane = self.panes.get_mut(self.includes).unwrap();
        pane.items.clear();
        pane.items.extend(
            self.config
                .include
                .iter()
                .enumerate()
                .map(|(i, s)| ListItem::new(ListState::Include, s.to_string(), i, false)),
        );
    }

    fn refresh_excludes(&mut self) {
        let pane = self.panes.get_mut(self.excludes).unwrap();
        pane.items.clear();
        pane.items.extend(
            self.config
                .exclude
                .iter()
                .enumerate()
                .map(|(i, s)| ListItem::new(ListState::Exclude, s.to_string(), i, false)),
        );
    }

    fn refresh_filters(&mut self) {
        let pane = self.panes.get_mut(self.filters).unwrap();
        pane.items.clear();
        pane.items.extend(
            self.config
                .regex
                .iter()
                .enumerate()
                .map(|(i, s)| ListItem::edit(s.to_string(), i)),
        );
    }
}

enum ConfigPane {
    Files,
    Includes,
    Excludes,
    Filters,
}

struct Pane {
    content: ConfigPane,
    items: Vec<ListItem>,
}

impl Pane {
    fn new(content: ConfigPane) -> Self {
        Self {
            content,
            items: vec![],
        }
    }

    fn content(&self) -> pane_grid::Content<Message> {
        let content = presets::column_list2(self.items.iter().map(|i| i.view()).collect());
        match self.content {
            ConfigPane::Files => presets::scroll_pane(
                "Files",
                Some(("Open", Message::FolderDialog)),
                content.into(),
            ),
            ConfigPane::Includes => presets::scroll_pane("Includes", None, content.into()),
            ConfigPane::Excludes => presets::scroll_pane("Excludes", None, content.into()),
            ConfigPane::Filters => {
                presets::scroll_pane("Filters", Some(("Add", Message::FilterAdd)), content.into())
            }
        }
    }
}

enum ListState {
    File,
    Folder,
    ParentFolder(bool),
    Include,
    Exclude,
    Filter,
    Error,
}

struct ListItem {
    state: ListState,
    index: usize,
    status: bool,
    text: String,
}

impl ListItem {
    fn new(state: ListState, text: String, index: usize, status: bool) -> Self {
        Self {
            state,
            index,
            status,
            text,
        }
    }

    fn error(text: String) -> Self {
        Self::new(ListState::Error, text, 0, false)
    }

    fn file(text: String, included: bool, is_dir: bool, index: usize) -> Self {
        if is_dir {
            Self::new(ListState::Folder, text, index, included)
        } else {
            Self::new(ListState::File, text, index, included)
        }
    }

    fn edit(text: String, index: usize) -> Self {
        let valid = text.is_empty() || Regex::new(&text).is_ok();
        Self::new(ListState::Filter, text, index, valid)
    }

    fn view(&self) -> Element<Message> {
        let row = presets::row_list();
        let row = match self.state {
            ListState::File => row.push(presets::space_icon()),
            ListState::Folder => row.push(presets::tooltip_right(
                presets::button_icon(">", Message::FolderOpen(self.index), false),
                "Open",
            )),
            ListState::ParentFolder(up) => row.push(presets::tooltip_right(
                presets::button_icon(
                    "<",
                    if up { Message::FolderUp } else { Message::None },
                    true,
                ),
                "Go Up",
            )),
            ListState::Include => row.push(presets::tooltip_right(
                presets::button_icon("O", Message::IncludeOpen(self.index), false),
                "Open",
            )),
            ListState::Exclude => row.push(presets::tooltip_right(
                presets::button_icon("O", Message::ExcludeOpen(self.index), false),
                "Open",
            )),
            ListState::Error | ListState::Filter => row,
        };
        let row = match &self.state {
            ListState::Error => row.push(presets::text_error(&self.text).width(Length::Fill)),
            ListState::Filter => row,
            _ => row.push(presets::text(&self.text).width(Length::Fill)),
        };
        let row = match &self.state {
            ListState::File | ListState::Folder | ListState::ParentFolder(_) => row
                .push(presets::tooltip_left(
                    presets::button_icon(
                        "+",
                        if self.status {
                            Message::None
                        } else {
                            Message::IncludeAdd(self.index)
                        },
                        false,
                    ),
                    "Include",
                ))
                .push(presets::tooltip_left(
                    presets::button_icon(
                        "-",
                        if self.status {
                            Message::ExcludeAdd(self.index)
                        } else {
                            Message::None
                        },
                        true,
                    ),
                    "Exclude",
                )),
            ListState::Include => row.push(presets::tooltip_left(
                presets::button_icon("-", Message::IncludeRemove(self.index), true),
                "Remove",
            )),
            ListState::Exclude => row.push(presets::tooltip_left(
                presets::button_icon("-", Message::ExcludeRemove(self.index), true),
                "Remove",
            )),
            ListState::Filter => {
                let i = self.index;
                let mess = move |t| Message::FilterEdit(i, t);
                let row = row.push(presets::regex_field(
                    &self.text,
                    "Regex filter",
                    self.status,
                    mess,
                ));
                if !self.status {
                    row.push(presets::text_error("Invalid"))
                } else {
                    row
                }
                .push(presets::tooltip_left(
                    presets::button_icon("-", Message::FilterRemove(self.index), true),
                    "Remove",
                ))
            }
            ListState::Error => row,
        };
        let row = row.push(presets::space_scroll());
        row.into()
    }
}
