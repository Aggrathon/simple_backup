#![cfg(feature = "gui")]
/// This module contains the logic for running the program through a GUI
use crate::{
    backup::{BackupError, BackupWriter},
    config::Config,
    files::{FileCrawler, FileInfo},
    utils::get_config_from_pathbuf,
};
use iced::{
    button, executor, pane_grid, pick_list, scrollable, text_input, Align, Application, Checkbox,
    Column, Command, Element, Length, PaneGrid, PickList, Row, Scrollable, Settings, Space,
    Subscription, Text,
};
use number_prefix::NumberPrefix;
use regex::Regex;
use rfd::{FileDialog, MessageDialog};
use std::{path::PathBuf, sync::mpsc::Receiver, thread::JoinHandle};

pub fn gui() {
    ApplicationState::run(Settings::default()).unwrap();
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    Main,
    CreateConfig,
    EditConfig,
    Backup,
    Restore,
    ToggleIncremental(bool),
    ThreadCount(u32),
    CompressionQuality(i32),
    AddInclude(usize),
    RemoveInclude(usize),
    CopyInclude(usize),
    AddExclude(usize),
    RemoveExclude(usize),
    CopyExclude(usize),
    AddFilter,
    RemoveFilter(usize),
    EditFilter(usize, String),
    OpenFolder(usize),
    GoUp,
    DialogFolder,
    SaveConfig,
    SortName,
    SortSize,
    SortTime,
    StartBackup,
    CancelBackup,
    Export,
    Tick,
    None,
}

enum ApplicationState {
    Main(MainState),
    Config(ConfigState),
    Backup(BackupState),
    Restore(RestoreState),
}

fn open_config() -> Option<Config> {
    FileDialog::new()
        .set_directory(dirs::home_dir().unwrap_or_default())
        .set_title("Open existing config or backup file")
        .add_filter("Config and backup files", &["yml", "tar.zst"])
        .add_filter("Config files", &["yml"])
        .add_filter("Backup files", &["tar.zst"])
        .pick_file()
        .and_then(|file| match get_config_from_pathbuf(file) {
            Ok(config) => Some(config),
            Err(e) => {
                MessageDialog::new()
                    .set_description(&e.to_string())
                    .set_level(rfd::MessageLevel::Error)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .set_title("Problem with reading config")
                    .show();
                None
            }
        })
}

impl Application for ApplicationState {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (ApplicationState::Main(MainState::new()), Command::none())
    }

    fn title(&self) -> String {
        match self {
            ApplicationState::Main(_) => String::from("simple_backup"),
            ApplicationState::Config(_) => String::from("simple_backup - Config"),
            ApplicationState::Backup(_) => String::from("simple_backup - Backup"),
            ApplicationState::Restore(_) => String::from("simple_backup - Restore"),
        }
    }

    fn update(
        &mut self,
        message: Self::Message,
        clipboard: &mut iced::Clipboard,
    ) -> iced::Command<Self::Message> {
        match message {
            Message::CreateConfig => {
                *self = ApplicationState::Config(ConfigState::new(true));
                Command::none()
            }
            Message::EditConfig => {
                if let ApplicationState::Backup(state) = self {
                    *self = ApplicationState::Config(ConfigState::from(std::mem::take(
                        &mut state.config,
                    )))
                } else if let Some(config) = open_config() {
                    *self = ApplicationState::Config(ConfigState::from(config))
                }
                Command::none()
            }
            Message::Backup => {
                if let ApplicationState::Config(state) = self {
                    //TODO Check for origin or open_folder
                    *self = ApplicationState::Backup(BackupState::new(std::mem::take(
                        &mut state.config,
                    )))
                } else if let ApplicationState::Backup(state) = self {
                    *self = ApplicationState::Backup(BackupState::new(std::mem::take(
                        &mut state.config,
                    )))
                } else if let Some(config) = open_config() {
                    *self = ApplicationState::Backup(BackupState::new(config))
                };
                Command::none()
            }
            Message::Restore => {
                *self = ApplicationState::Restore(RestoreState {});
                Command::none()
            }
            Message::None => {
                eprintln!("Unspecified GUI message");
                Command::none()
            }
            Message::Main => {
                *self = ApplicationState::Main(MainState::new());
                Command::none()
            }
            _ => match self {
                ApplicationState::Main(_) => Command::none(),
                ApplicationState::Config(state) => state.update(message, clipboard),
                ApplicationState::Backup(state) => state.update(message, clipboard),
                ApplicationState::Restore(_) => todo!(),
            },
        }
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        match self {
            ApplicationState::Main(state) => state.view(),
            ApplicationState::Config(state) => state.view(),
            ApplicationState::Backup(state) => state.view(),
            ApplicationState::Restore(_) => todo!(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match self {
            ApplicationState::Backup(state) => state.subscription(),
            _ => Subscription::none(),
        }
    }
}

struct MainState {
    create: button::State,
    edit: button::State,
    backup: button::State,
    config: button::State,
}

impl MainState {
    fn new() -> Self {
        Self {
            create: button::State::new(),
            edit: button::State::new(),
            backup: button::State::new(),
            config: button::State::new(),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let col = Column::with_children(vec![
            Space::with_height(Length::Fill).into(),
            presets::text_title("simple_backup").into(),
            Space::with_height(Length::Shrink).into(),
            presets::button_main(&mut self.create, "Create", Message::CreateConfig).into(),
            presets::button_main(&mut self.edit, "Edit", Message::EditConfig).into(),
            presets::button_main(&mut self.backup, "Backup", Message::Backup).into(),
            presets::button_main(&mut self.config, "Restore", Message::Restore).into(),
            Space::with_height(Length::Fill).into(),
        ])
        .align_items(Align::Center)
        .spacing(presets::LARGE_SPACING);
        Row::with_children(vec![
            Space::with_width(Length::Fill).into(),
            col.into(),
            Space::with_width(Length::Fill).into(),
        ])
        .into()
    }
}

struct ConfigState {
    config: Config,
    panes: pane_grid::State<Pane>,
    back: button::State,
    save: button::State,
    backup: button::State,
    threads: pick_list::State<u32>,
    thread_alt: Vec<u32>,
    compression: pick_list::State<i32>,
    compression_alt: Vec<i32>,
    files: pane_grid::Pane,
    includes: pane_grid::Pane,
    excludes: pane_grid::Pane,
    filters: pane_grid::Pane,
    current_dir: FileInfo,
}

impl ConfigState {
    fn new(open_home: bool) -> Self {
        let (mut panes, files) = pane_grid::State::new(Pane::new(ConfigPane::Files));
        let (includes, _) = panes
            .split(
                pane_grid::Axis::Vertical,
                &files,
                Pane::new(ConfigPane::Includes),
            )
            .unwrap();
        let (excludes, _) = panes
            .split(
                pane_grid::Axis::Horizontal,
                &includes,
                Pane::new(ConfigPane::Excludes),
            )
            .unwrap();
        let (filters, _) = panes
            .split(
                pane_grid::Axis::Horizontal,
                &excludes,
                Pane::new(ConfigPane::Filters),
            )
            .unwrap();
        let mut state = Self {
            config: Config::new(),
            panes,
            back: button::State::new(),
            save: button::State::new(),
            backup: button::State::new(),
            threads: pick_list::State::default(),
            thread_alt: (1u32..num_cpus::get() as u32 + 1).collect(),
            compression: pick_list::State::default(),
            compression_alt: (1..23).collect(),
            files,
            includes,
            excludes,
            filters,
            current_dir: FileInfo::from(dirs::home_dir().unwrap_or_default()),
        };
        if open_home {
            state.refresh_files();
        }
        state
    }

    fn from(mut config: Config) -> Self {
        config.sort();
        let mut state = Self::new(false);
        state.current_dir = FileInfo::from(config.get_dir());
        state.config = config;
        state.refresh_includes();
        state.refresh_excludes();
        state.refresh_filters();
        state.refresh_files();
        state
    }

    fn view(&mut self) -> Element<Message> {
        let pane_grid = PaneGrid::new(&mut self.panes, |_, pane| pane.content())
            .on_resize(10, Message::PaneResized)
            .on_drag(Message::PaneDragged)
            .spacing(presets::OUTER_SPACING);
        let bar = Row::with_children(vec![
            presets::button_nav(&mut self.back, "Back", Message::Main, false).into(),
            Space::with_width(Length::Fill).into(),
            PickList::new(
                &mut self.threads,
                &self.thread_alt,
                Some(self.config.threads),
                Message::ThreadCount,
            )
            .into(),
            Text::new(if self.config.threads > 1 {
                " Threads"
            } else {
                " Thread "
            })
            .into(),
            Space::with_width(Length::Units(presets::LARGE_SPACING)).into(),
            PickList::new(
                &mut self.compression,
                &self.compression_alt,
                Some(self.config.quality),
                Message::CompressionQuality,
            )
            .into(),
            Text::new("Compression quality").into(),
            Space::with_width(Length::Units(presets::LARGE_SPACING)).into(),
            Checkbox::new(
                self.config.incremental,
                "Incremental backups",
                Message::ToggleIncremental,
            )
            .into(),
            Space::with_width(Length::Fill).into(),
            presets::button_nav(&mut self.save, "Save", Message::SaveConfig, true).into(),
            presets::button_nav(&mut self.backup, "Backup", Message::Backup, true).into(),
        ])
        .spacing(presets::INNER_SPACING)
        .align_items(Align::Center);
        Column::with_children(vec![pane_grid.into(), bar.into()])
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING)
            .padding(presets::INNER_SPACING)
            .into()
    }

    fn update(
        &mut self,
        message: Message,
        clipboard: &mut iced::Clipboard,
    ) -> iced::Command<Message> {
        match message {
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(&split, ratio)
            }
            Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.swap(&pane, &target)
            }
            Message::PaneDragged(_) => {}
            Message::ToggleIncremental(t) => self.config.incremental = t,
            Message::ThreadCount(text) => self.config.set_threads(text),
            Message::CompressionQuality(text) => self.config.set_quality(text),
            Message::AddInclude(i) => {
                let pane = self.panes.get_mut(&self.files).unwrap();
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
            Message::RemoveInclude(i) => {
                if i < self.config.include.len() {
                    self.config.include.remove(i);
                    self.refresh_includes();
                    self.refresh_files();
                }
            }
            Message::CopyInclude(i) => {
                if let Some(s) = self.config.include.get(i) {
                    clipboard.write(s.to_string());
                }
            }
            Message::AddExclude(i) => {
                let pane = self.panes.get_mut(&self.files).unwrap();
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
            Message::RemoveExclude(i) => {
                if i < self.config.exclude.len() {
                    self.config.exclude.remove(i);
                    self.refresh_excludes();
                    self.refresh_files();
                }
            }
            Message::CopyExclude(i) => {
                if let Some(s) = self.config.exclude.get(i) {
                    clipboard.write(s.to_string());
                }
            }
            Message::AddFilter => {
                self.config.regex.push(String::new());
                self.refresh_filters();
            }
            Message::RemoveFilter(i) => {
                if i < self.config.regex.len() {
                    self.config.regex.remove(i);
                    self.refresh_filters();
                    self.refresh_files();
                }
            }
            Message::EditFilter(i, s) => {
                let pane = self.panes.get_mut(&self.filters).unwrap();
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
            Message::OpenFolder(i) => {
                let pane = self.panes.get_mut(&self.files).unwrap();
                if let Some(li) = pane.items.get_mut(i) {
                    self.current_dir = FileInfo::from(std::mem::take(&mut li.text));
                    self.refresh_files();
                }
            }
            Message::DialogFolder => {
                if let Some(folder) = FileDialog::new()
                    .set_directory(self.current_dir.get_path())
                    .set_title("Open Directory")
                    .pick_folder()
                {
                    self.current_dir = FileInfo::from(folder);
                    self.refresh_files();
                }
            }
            Message::GoUp => {
                if let Some(dir) = self.current_dir.get_path().parent() {
                    self.current_dir = FileInfo::from(dir);
                    self.refresh_files();
                }
            }
            Message::SaveConfig => {
                if let Some(file) = FileDialog::new()
                    .set_directory(
                        self.config
                            .origin
                            .as_ref()
                            .and_then(|s| Some(PathBuf::from(s)))
                            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default()),
                    )
                    .set_title("Save config file")
                    .set_file_name("config.yml")
                    .add_filter("Config file", &["yml"])
                    .save_file()
                {
                    match self.config.write_yaml(file) {
                        Ok(_) => {}
                        Err(e) => {
                            MessageDialog::new()
                                .set_description(&e.to_string())
                                .set_level(rfd::MessageLevel::Error)
                                .set_buttons(rfd::MessageButtons::Ok)
                                .set_title("Problem saving config")
                                .show();
                        }
                    };
                }
            }
            _ => eprintln!("Unexpected GUI message"),
        }
        Command::none()
    }

    fn refresh_files(&mut self) {
        let pane = self.panes.get_mut(&self.files).unwrap();
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
        let pane = self.panes.get_mut(&self.includes).unwrap();
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
        let pane = self.panes.get_mut(&self.excludes).unwrap();
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
        let pane = self.panes.get_mut(&self.filters).unwrap();
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
    scroll: scrollable::State,
    top_button: button::State,
    items: Vec<ListItem>,
}

impl Pane {
    fn new(content: ConfigPane) -> Self {
        Self {
            content,
            scroll: scrollable::State::new(),
            top_button: button::State::new(),
            items: vec![],
        }
    }

    fn content(&mut self) -> pane_grid::Content<Message> {
        let content = Scrollable::new(&mut self.scroll)
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING)
            .padding(presets::OUTER_SPACING);
        let content = self
            .items
            .iter_mut()
            .fold(content, |content, item| content.push(item.view()));
        match self.content {
            ConfigPane::Files => presets::pane_border(
                "Files",
                Some(("Open", &mut self.top_button, Message::DialogFolder)),
                content.into(),
            ),
            ConfigPane::Includes => presets::pane_border("Includes", None, content.into()),
            ConfigPane::Excludes => presets::pane_border("Excludes", None, content.into()),
            ConfigPane::Filters => presets::pane_border(
                "Filters",
                Some(("Add", &mut self.top_button, Message::AddFilter)),
                content.into(),
            ),
        }
    }
}

enum ListState {
    File,
    Folder,
    ParentFolder(bool),
    Include,
    Exclude,
    Filter(text_input::State),
    Error,
}

struct ListItem {
    state: ListState,
    index: usize,
    status: bool,
    text: String,
    open_state: button::State,
    add_state: button::State,
    remove_state: button::State,
}

impl ListItem {
    fn new(state: ListState, text: String, index: usize, status: bool) -> Self {
        Self {
            state,
            index,
            status,
            text,
            open_state: button::State::new(),
            add_state: button::State::new(),
            remove_state: button::State::new(),
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
        Self::new(
            ListState::Filter(text_input::State::new()),
            text,
            index,
            valid,
        )
    }

    fn view(&mut self) -> Element<Message> {
        let row = Row::new()
            .width(Length::Fill)
            // .padding(presets::OUTER_SPACING)
            .spacing(presets::INNER_SPACING)
            .align_items(Align::Center);
        let row = match self.state {
            ListState::File => row.push(presets::space_icon()),
            ListState::Folder => row.push(presets::tooltip_right(
                presets::button_icon(
                    &mut self.open_state,
                    ">",
                    Message::OpenFolder(self.index),
                    false,
                )
                .into(),
                "Open",
            )),
            ListState::ParentFolder(up) => row.push(presets::tooltip_right(
                presets::button_icon(
                    &mut self.open_state,
                    "<",
                    if up { Message::GoUp } else { Message::None },
                    true,
                )
                .into(),
                "Go Up",
            )),
            ListState::Include => row.push(presets::tooltip_right(
                presets::button_icon(
                    &mut self.open_state,
                    "C",
                    Message::CopyInclude(self.index),
                    false,
                )
                .into(),
                "Copy",
            )),
            ListState::Exclude => row.push(presets::tooltip_right(
                presets::button_icon(
                    &mut self.open_state,
                    "C",
                    Message::CopyExclude(self.index),
                    false,
                )
                .into(),
                "Copy",
            )),
            ListState::Error | ListState::Filter(..) => row,
        };
        let row = match &mut self.state {
            ListState::Error => row.push(presets::text_error(&self.text).width(Length::Fill)),
            ListState::Filter(..) => row,
            _ => row.push(Text::new(&self.text).width(Length::Fill)),
        };
        let row = match &mut self.state {
            ListState::File | ListState::Folder | ListState::ParentFolder(_) => row
                .push(presets::tooltip_left(
                    presets::button_icon(
                        &mut self.add_state,
                        "+",
                        if self.status {
                            Message::None
                        } else {
                            Message::AddInclude(self.index)
                        },
                        false,
                    )
                    .into(),
                    "Include",
                ))
                .push(presets::tooltip_left(
                    presets::button_icon(
                        &mut self.remove_state,
                        "-",
                        if self.status {
                            Message::AddExclude(self.index)
                        } else {
                            Message::None
                        },
                        true,
                    )
                    .into(),
                    "Exclude",
                )),
            ListState::Include => row.push(presets::tooltip_left(
                presets::button_icon(
                    &mut self.remove_state,
                    "-",
                    Message::RemoveInclude(self.index),
                    true,
                )
                .into(),
                "Remove",
            )),
            ListState::Exclude => row.push(presets::tooltip_left(
                presets::button_icon(
                    &mut self.remove_state,
                    "-",
                    Message::RemoveExclude(self.index),
                    true,
                )
                .into(),
                "Remove",
            )),
            ListState::Filter(state) => {
                let i = self.index;
                let mess = move |t| Message::EditFilter(i, t);
                let row = row.push(presets::regex_field(
                    state,
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
                    presets::button_icon(
                        &mut self.remove_state,
                        "-",
                        Message::RemoveFilter(self.index),
                        true,
                    )
                    .into(),
                    "Remove",
                ))
            }
            ListState::Error => row,
        };
        let row = row.push(presets::space_scroll());
        row.into()
    }
}

struct ThreadWrapper<T1, T2> {
    queue: Receiver<T1>,
    handle: JoinHandle<T2>,
}

impl From<Config> for ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter> {
    fn from(config: Config) -> Self {
        let (send, queue) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let (mut writer, error) = BackupWriter::new(config);
            #[allow(unused_must_use)]
            if let Some(e) = error {
                send.send(Err(e));
            }
            let error = writer.iter_files(false, |res| {
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
}

#[derive(PartialEq, Eq)]
enum ListSort {
    Name,
    Size,
    Time,
}

enum BackupStage {
    Scanning(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Viewing(BackupWriter),
    Performing(BackupWriter),
    None,
}

struct BackupState {
    config: Config,
    scroll_state: scrollable::State,
    edit_button: button::State,
    backup_button: button::State,
    name_button: button::State,
    size_button: button::State,
    time_button: button::State,
    export_button: button::State,
    list_sort: ListSort,
    error: String,
    count: u64,
    size: u64,
    stage: BackupStage,
}

impl BackupState {
    fn new(config: Config) -> Self {
        let crawler = ThreadWrapper::from(config.clone());
        Self {
            config,
            scroll_state: scrollable::State::new(),
            edit_button: button::State::new(),
            backup_button: button::State::new(),
            name_button: button::State::new(),
            size_button: button::State::new(),
            time_button: button::State::new(),
            export_button: button::State::new(),
            list_sort: ListSort::Name,
            error: String::new(),
            count: 0,
            size: 0,
            stage: BackupStage::Scanning(crawler),
        }
    }

    fn update(&mut self, message: Message, _clipboard: &mut iced::Clipboard) -> Command<Message> {
        match message {
            Message::Tick => {
                if let BackupStage::Scanning(crawler) = &mut self.stage {
                    for _ in 0..1000 {
                        match crawler.queue.try_recv() {
                            Ok(res) => match res {
                                Ok(fi) => {
                                    self.count += 1;
                                    self.size += fi.size
                                }
                                Err(e) => {
                                    self.error.push('\n');
                                    self.error.push_str(&e.to_string())
                                }
                            },
                            Err(e) => match e {
                                std::sync::mpsc::TryRecvError::Empty => {
                                    break;
                                }
                                std::sync::mpsc::TryRecvError::Disconnected => {
                                    if let BackupStage::Scanning(crawler) =
                                        std::mem::replace(&mut self.stage, BackupStage::None)
                                    {
                                        match crawler.handle.join() {
                                            Ok(bw) => self.stage = BackupStage::Viewing(bw),
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
            }
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
            Message::StartBackup => {
                if let BackupStage::Viewing(_) = &self.stage {
                    self.list_sort = ListSort::Name;
                    if let BackupStage::Viewing(mut writer) =
                        std::mem::replace(&mut self.stage, BackupStage::None)
                    {
                        writer.list.as_mut().unwrap().sort_unstable();
                        self.stage = BackupStage::Performing(writer);
                        // TODO backup
                    }
                }
            }
            Message::CancelBackup => {
                if let BackupStage::Performing(_) = &self.stage {
                    if let BackupStage::Performing(mut writer) =
                        std::mem::replace(&mut self.stage, BackupStage::None)
                    {
                        writer.list.as_mut().unwrap().sort_unstable();
                        self.stage = BackupStage::Viewing(writer);
                        //TODO cancel backup
                    }
                }
            }
            Message::Export => {
                if let BackupStage::Viewing(writer) = &mut self.stage {
                    if let Some(file) = FileDialog::new()
                        .set_directory(
                            self.config
                                .origin
                                .as_ref()
                                .and_then(|s| Some(PathBuf::from(s)))
                                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default()),
                        )
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
            _ => eprintln!("Unexpected GUI message"),
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.stage {
            BackupStage::Scanning(_) | BackupStage::Performing(_) => {
                iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::Tick)
            }
            _ => Subscription::none(),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let mut scroll = Scrollable::new(&mut self.scroll_state)
            .height(Length::Fill)
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING);
        let status = match NumberPrefix::binary(self.size as f64) {
            NumberPrefix::Standalone(number) => {
                format!(
                    "{} files of total size {:.2} KiB",
                    self.count,
                    number / 1024.0
                )
            }
            NumberPrefix::Prefixed(prefix, number) => {
                format!(
                    "{} files of total size {:.2} {}B",
                    self.count, number, prefix
                )
            }
        };
        match &mut self.stage {
            BackupStage::Scanning(_) => {
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error)
                            .horizontal_alignment(iced::HorizontalAlignment::Left),
                    );
                }
                let brow = Row::with_children(vec![
                    presets::button_nav(&mut self.edit_button, "Edit", Message::EditConfig, false)
                        .into(),
                    Space::with_width(Length::Fill).into(),
                    Text::new(format!("Scanning for files to backup: {}\n", status))
                        .vertical_alignment(iced::VerticalAlignment::Center)
                        .into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav(&mut self.backup_button, "Backup", Message::None, true)
                        .into(),
                ])
                .spacing(presets::INNER_SPACING);
                Column::with_children(vec![scroll.into(), brow.into()])
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::INNER_SPACING)
                    .into()
            }
            BackupStage::Viewing(writer) => {
                let trow = Row::with_children(vec![
                    presets::button_grey(
                        &mut self.name_button,
                        "Name",
                        Message::SortName,
                        self.list_sort != ListSort::Name,
                    )
                    .width(Length::Fill)
                    .into(),
                    presets::button_grey(
                        &mut self.size_button,
                        "Size",
                        Message::SortSize,
                        self.list_sort != ListSort::Size,
                    )
                    .width(Length::Units(102))
                    .into(),
                    presets::button_grey(
                        &mut self.time_button,
                        "Time",
                        Message::SortTime,
                        self.list_sort != ListSort::Time,
                    )
                    .width(Length::Units(182))
                    .into(),
                ])
                .spacing(presets::INNER_SPACING);
                scroll = writer
                    .list
                    .as_mut()
                    .unwrap()
                    .iter_mut()
                    .take(100)
                    .fold(scroll, |s, f| {
                        s.push(
                            Row::with_children(vec![
                                Text::new(f.get_string()).width(Length::Fill).into(),
                                Text::new(match NumberPrefix::binary(f.size as f64) {
                                    NumberPrefix::Standalone(num) => {
                                        format!("{:>6.0}  B", num / 1024.0)
                                    }
                                    NumberPrefix::Prefixed(pre, num) => {
                                        format!("{:>6.2} {}B", num, pre)
                                    }
                                })
                                .width(Length::Units(102))
                                .horizontal_alignment(iced::HorizontalAlignment::Right)
                                .into(),
                                Text::new(f.time.unwrap().format("%Y-%m-%d %H:%M:%S").to_string())
                                    .width(Length::Units(182))
                                    .horizontal_alignment(iced::HorizontalAlignment::Right)
                                    .into(),
                                presets::space_scroll().into(),
                            ])
                            .spacing(presets::INNER_SPACING),
                        )
                    });
                if writer.list.as_ref().unwrap().len() > 100 {
                    scroll = scroll.push(
                        Row::with_children(vec![
                            Space::with_width(Length::Fill).into(),
                            presets::button_color(
                                &mut self.export_button,
                                "Export full list",
                                Message::Export,
                            )
                            .into(),
                            Space::with_width(Length::Fill).into(),
                        ])
                        .spacing(presets::INNER_SPACING),
                    );
                }
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error)
                            .horizontal_alignment(iced::HorizontalAlignment::Left),
                    );
                }
                let brow = Row::with_children(vec![
                    presets::button_nav(&mut self.edit_button, "Edit", Message::EditConfig, false)
                        .into(),
                    Space::with_width(Length::Fill).into(),
                    Text::new(&status)
                        .vertical_alignment(iced::VerticalAlignment::Center)
                        .into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav(
                        &mut self.backup_button,
                        "Backup",
                        Message::StartBackup,
                        true,
                    )
                    .into(),
                ]);
                Column::with_children(vec![trow.into(), scroll.into(), brow.into()])
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::INNER_SPACING)
                    .into()
            }
            BackupStage::Performing(_) => {
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error)
                            .horizontal_alignment(iced::HorizontalAlignment::Left),
                    );
                }
                let max = (self.size / 1024 + self.count) as f32;
                let current = (self.size / 1024) as f32; // TODO Use correct current
                let bar = presets::progress_bar(current, max);
                let brow = Row::with_children(vec![
                    presets::button_nav(&mut self.edit_button, "Edit", Message::None, false).into(),
                    Space::with_width(Length::Fill).into(),
                    Text::new(&format!("Backing up {}", status))
                        .vertical_alignment(iced::VerticalAlignment::Center)
                        .into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav(
                        &mut self.backup_button,
                        "Cancel",
                        Message::CancelBackup,
                        false,
                    )
                    .into(),
                ])
                .spacing(presets::INNER_SPACING);
                Column::with_children(vec![scroll.into(), bar.into(), brow.into()])
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::INNER_SPACING)
                    .into()
            }
            BackupStage::None => {
                presets::text_error("This should not be possible: `stage == None`").into()
            }
        }
    }
}

struct RestoreState {
    //TODO Restore GUI
}

mod presets {
    use iced::{
        button, container, pane_grid, progress_bar, text_input, tooltip, Button, Color, Element,
        Length, ProgressBar, Row, Space, Text, TextInput, Tooltip,
    };

    use super::Message;

    const APP_COLOR: Color = Color::from_rgb(78.0 / 255.0, 155.0 / 255.0, 71.0 / 255.0); //#4E9B47
    const APP2_COLOR: Color = Color::from_rgb(172.0 / 255.0, 215.0 / 255.0, 168.0 / 255.0); //#acd7a8
    const COMP_COLOR: Color = Color::from_rgb(148.0 / 255.0, 71.0 / 255.0, 155.0 / 255.0); //#94479b
    const GREY_COLOR: Color = Color::from_rgb(0.65, 0.65, 0.65);
    const LIGHT_COLOR: Color = Color::from_rgb(0.9, 0.9, 0.9);
    const DARK_COLOR: Color = Color::from_rgb(0.3, 0.3, 0.3);
    const SMALL_RADIUS: f32 = 3.0;
    const LARGE_RADIUS: f32 = 5.0;
    const ICON_BUTTON_WIDTH: u16 = 30;
    pub const INNER_SPACING: u16 = 3;
    pub const OUTER_SPACING: u16 = 6;
    pub const LARGE_SPACING: u16 = 6;

    pub(crate) fn button_color<'a>(
        state: &'a mut button::State,
        text: &str,
        action: Message,
    ) -> Button<'a, Message> {
        let label = Text::new(text)
            .horizontal_alignment(iced::HorizontalAlignment::Center)
            .vertical_alignment(iced::VerticalAlignment::Center);
        let but = Button::new(state, label).style(ButtonStyle::ColorButton);
        if let Message::None = action {
            but
        } else {
            but.on_press(action)
        }
    }
    pub(crate) fn button_grey<'a>(
        state: &'a mut button::State,
        text: &str,
        action: Message,
        light: bool,
    ) -> Button<'a, Message> {
        let label = Text::new(text)
            .horizontal_alignment(iced::HorizontalAlignment::Center)
            .vertical_alignment(iced::VerticalAlignment::Center);
        let but = if light {
            Button::new(state, label).style(ButtonStyle::LightButton)
        } else {
            Button::new(state, label).style(ButtonStyle::GreyButton)
        };
        if let Message::None = action {
            but
        } else {
            but.on_press(action)
        }
    }

    pub(crate) fn button_nav<'a>(
        state: &'a mut button::State,
        text: &str,
        action: Message,
        forward: bool,
    ) -> Button<'a, Message> {
        let label = Text::new(text)
            .width(Length::Units(64))
            .horizontal_alignment(iced::HorizontalAlignment::Center)
            .vertical_alignment(iced::VerticalAlignment::Center);
        let but = Button::new(state, label).style(if forward {
            ButtonStyle::ColorButton
        } else {
            ButtonStyle::NegativeButton
        });
        if let Message::None = action {
            but
        } else {
            but.on_press(action)
        }
    }

    pub(crate) fn button_icon<'a>(
        state: &'a mut button::State,
        text: &str,
        action: Message,
        negative: bool,
    ) -> Button<'a, Message> {
        let label = Text::new(text)
            .horizontal_alignment(iced::HorizontalAlignment::Center)
            .vertical_alignment(iced::VerticalAlignment::Center);
        let but = Button::new(state, label)
            .style(if negative {
                ButtonStyle::NegativeButton
            } else {
                ButtonStyle::ColorButton
            })
            .width(Length::Units(ICON_BUTTON_WIDTH));
        if let Message::None = action {
            but
        } else {
            but.on_press(action)
        }
    }

    pub(crate) fn space_icon() -> Space {
        Space::with_width(Length::Units(ICON_BUTTON_WIDTH))
    }

    pub(crate) fn space_scroll() -> Space {
        Space::with_width(Length::Units(0))
    }

    pub(crate) fn button_main<'a>(
        state: &'a mut button::State,
        text: &str,
        action: Message,
    ) -> Button<'a, Message> {
        let label = Text::new(text)
            .horizontal_alignment(iced::HorizontalAlignment::Center)
            .vertical_alignment(iced::VerticalAlignment::Center);
        let but = Button::new(state, label)
            .min_width(200)
            .min_height(40)
            .style(ButtonStyle::MainButton);
        if let Message::None = action {
            but
        } else {
            but.on_press(action)
        }
    }

    pub(crate) fn text_title(text: &str) -> Text {
        Text::new(text)
            .size(32)
            .horizontal_alignment(iced::HorizontalAlignment::Center)
    }

    pub(crate) fn text_error(text: &str) -> Text {
        Text::new(text)
            .color(COMP_COLOR)
            .horizontal_alignment(iced::HorizontalAlignment::Center)
    }

    pub(crate) fn pane_border<'a>(
        title: &str,
        button: Option<(&str, &'a mut button::State, Message)>,
        content: Element<'a, Message>,
    ) -> pane_grid::Content<'a, Message> {
        let title = Row::with_children(vec![
            Space::with_width(Length::Shrink).into(),
            Text::new(title)
                .vertical_alignment(iced::VerticalAlignment::Center)
                .into(),
        ])
        .align_items(iced::Align::Center)
        .spacing(INNER_SPACING)
        .padding(INNER_SPACING);
        let mut title_bar = pane_grid::TitleBar::new(title).style(ContainerStyle::PaneTitleBar);
        if let Some((text, state, action)) = button {
            title_bar = title_bar
                .controls(button_color(state, text, action))
                .always_show_controls();
        }
        pane_grid::Content::new(content)
            .title_bar(title_bar)
            .style(ContainerStyle::Pane)
    }

    pub(crate) fn tooltip_right<'a>(
        content: Element<'a, Message>,
        tip: &str,
    ) -> Tooltip<'a, Message> {
        Tooltip::new(content, tip, tooltip::Position::Right).style(ContainerStyle::Tooltip)
    }

    pub(crate) fn tooltip_left<'a>(
        content: Element<'a, Message>,
        tip: &str,
    ) -> Tooltip<'a, Message> {
        Tooltip::new(content, tip, tooltip::Position::Left).style(ContainerStyle::Tooltip)
    }

    pub(crate) fn regex_field<'a, F>(
        state: &'a mut text_input::State,
        value: &'a String,
        placeholder: &str,
        valid_regex: bool,
        mess: F,
    ) -> TextInput<'a, Message>
    where
        F: 'static + Fn(String) -> Message,
    {
        let inp = TextInput::new(state, placeholder, value, mess).padding(LARGE_SPACING);
        if value.is_empty() {
            inp.style(InputStyle::Normal)
        } else if valid_regex {
            inp.style(InputStyle::Working)
        } else {
            inp.style(InputStyle::Problem)
        }
    }

    pub(crate) fn progress_bar(current: f32, max: f32) -> ProgressBar {
        ProgressBar::new(0.0..=max, current)
            .width(Length::Fill)
            .style(ProgressStyle::Normal)
    }

    pub enum ButtonStyle {
        GreyButton,
        LightButton,
        MainButton,
        ColorButton,
        NegativeButton,
    }

    pub enum ContainerStyle {
        PaneTitleBar,
        Pane,
        Tooltip,
    }

    pub enum InputStyle {
        Normal,
        Working,
        Problem,
    }

    pub enum ProgressStyle {
        Normal,
    }

    impl container::StyleSheet for ContainerStyle {
        fn style(&self) -> container::Style {
            match &self {
                ContainerStyle::PaneTitleBar => container::Style {
                    text_color: Some(Color::WHITE),
                    background: Some(GREY_COLOR.into()),
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                ContainerStyle::Pane => container::Style {
                    background: Some(Color::WHITE.into()),
                    border_width: 2.0,
                    border_color: GREY_COLOR,
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                ContainerStyle::Tooltip => container::Style {
                    background: Some(LIGHT_COLOR.into()),
                    border_radius: SMALL_RADIUS,
                    ..container::Style::default()
                },
            }
        }
    }

    impl button::StyleSheet for ButtonStyle {
        fn active(&self) -> button::Style {
            match &self {
                ButtonStyle::GreyButton => button::Style {
                    background: Some(GREY_COLOR.into()),
                    text_color: Color::WHITE,
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                ButtonStyle::LightButton => button::Style {
                    background: Some(LIGHT_COLOR.into()),
                    text_color: Color::BLACK,
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                ButtonStyle::MainButton => button::Style {
                    background: Some(APP_COLOR.into()),
                    text_color: Color::WHITE,
                    border_radius: LARGE_RADIUS,
                    ..Default::default()
                },
                ButtonStyle::ColorButton => button::Style {
                    background: Some(APP_COLOR.into()),
                    text_color: Color::WHITE,
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                ButtonStyle::NegativeButton => button::Style {
                    background: Some(COMP_COLOR.into()),
                    text_color: Color::WHITE,
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
            }
        }
    }

    impl text_input::StyleSheet for InputStyle {
        fn active(&self) -> text_input::Style {
            text_input::Style {
                background: Color::WHITE.into(),
                border_color: GREY_COLOR,
                border_radius: SMALL_RADIUS,
                border_width: 1.0,
                ..Default::default()
            }
        }

        fn focused(&self) -> text_input::Style {
            text_input::Style { ..self.active() }
        }

        fn placeholder_color(&self) -> Color {
            LIGHT_COLOR
        }

        fn value_color(&self) -> Color {
            match self {
                InputStyle::Normal => DARK_COLOR,
                InputStyle::Working => DARK_COLOR,
                InputStyle::Problem => COMP_COLOR,
            }
        }

        fn selection_color(&self) -> Color {
            APP2_COLOR
        }
    }

    impl progress_bar::StyleSheet for ProgressStyle {
        fn style(&self) -> progress_bar::Style {
            progress_bar::Style {
                background: GREY_COLOR.into(),
                bar: APP_COLOR.into(),
                border_radius: LARGE_RADIUS,
            }
        }
    }
}
