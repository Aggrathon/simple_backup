#![cfg(feature = "gui")]
/// This module contains the logic for running the program through a GUI
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use iced::pure::widget::{pane_grid, Column, PaneGrid, PickList, Row, Scrollable, Space};
use iced::pure::{Application, Element};
use iced::{
    alignment::Horizontal, alignment::Vertical, clipboard, executor, Alignment, Checkbox, Command,
    Length, Settings, Subscription, Text,
};
use regex::Regex;
use rfd::{FileDialog, MessageDialog};

use crate::backup::{BackupError, BackupReader, BackupWriter};
use crate::config::Config;
use crate::files::{FileCrawler, FileInfo};
use crate::utils::{format_size, get_config_from_pathbuf};

mod paginated;
mod presets;

#[allow(dead_code)]
#[cfg_attr(target_os = "windows", link(name = "Kernel32"))]
extern "system" {
    fn FreeConsole() -> i32;
}

pub fn gui() {
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    unsafe {
        FreeConsole()
    }; // Safety: Windows syscall to hide console
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
    GoTo(usize),
    StartBackup,
    CancelBackup,
    Export,
    Tick,
    ToggleSelected(usize),
    ToggleAll,
    RestoreAll,
    ExtractSelected,
    None,
}

enum ApplicationState<'a> {
    Main(MainState),
    Config(ConfigState),
    Backup(BackupState),
    Restore(RestoreState<'a>),
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
fn open_backup<'a>() -> Option<BackupReader<'a>> {
    FileDialog::new()
        .set_directory(dirs::home_dir().unwrap_or_default())
        .set_title("Open backup file")
        .add_filter("Backup files", &["tar.zst"])
        .pick_file()
        .and_then(|file| match BackupReader::read(file) {
            Ok(reader) => Some(reader),
            Err(e) => {
                MessageDialog::new()
                    .set_description(&e.to_string())
                    .set_level(rfd::MessageLevel::Error)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .set_title("Problem with reading backup")
                    .show();
                None
            }
        })
}

impl<'a> Application for ApplicationState<'a> {
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

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
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
                    let mut config = std::mem::take(&mut state.config);
                    if let Some(path) = FileDialog::new()
                        .set_directory(config.get_output_home())
                        .set_title("Where should the backups be stored")
                        .pick_folder()
                    {
                        config.output = path;
                        *self = ApplicationState::Backup(BackupState::new(config))
                    }
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
                if let Some(reader) = open_backup() {
                    *self = ApplicationState::Restore(RestoreState::new(reader));
                }
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
                ApplicationState::Config(state) => state.update(message),
                ApplicationState::Backup(state) => state.update(message),
                ApplicationState::Restore(state) => state.update(message),
            },
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        match self {
            ApplicationState::Main(state) => state.view(),
            ApplicationState::Config(state) => state.view(),
            ApplicationState::Backup(state) => state.view(),
            ApplicationState::Restore(state) => state.view(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match self {
            ApplicationState::Backup(state) => state.subscription(),
            _ => Subscription::none(),
        }
    }
}

struct MainState {}

impl MainState {
    fn new() -> Self {
        Self {}
    }

    fn view(&self) -> Element<Message> {
        let col = Column::with_children(vec![
            Space::with_height(Length::Fill).into(),
            presets::text_title("simple_backup").into(),
            Space::with_height(Length::Shrink).into(),
            presets::button_main("Create", false, Message::CreateConfig).into(),
            presets::button_main("Edit", false, Message::EditConfig).into(),
            presets::button_main("Backup", false, Message::Backup).into(),
            presets::button_main("Restore", true, Message::Restore).into(),
            Space::with_height(Length::Fill).into(),
        ])
        .align_items(Alignment::Center)
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
    thread_alt: Vec<u32>,
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
            thread_alt: (1u32..num_cpus::get() as u32 + 1).collect(),
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

    fn view(&self) -> Element<Message> {
        let pane_grid = PaneGrid::new(&self.panes, |_, pane| pane.content())
            .on_resize(10, Message::PaneResized)
            .on_drag(Message::PaneDragged)
            .spacing(presets::OUTER_SPACING);
        let bar = Row::with_children(vec![
            presets::button_nav("Back", Message::Main, false).into(),
            Space::with_width(Length::Fill).into(),
            Text::new("Threads:").into(),
            PickList::new(
                &self.thread_alt,
                Some(self.config.threads),
                Message::ThreadCount,
            )
            .into(),
            Space::with_width(Length::Units(presets::LARGE_SPACING)).into(),
            Text::new("Compression quality:").into(),
            PickList::new(
                &self.compression_alt,
                Some(self.config.quality),
                Message::CompressionQuality,
            )
            .into(),
            Space::with_width(Length::Units(presets::LARGE_SPACING)).into(),
            Checkbox::new(
                self.config.incremental,
                "Incremental backups",
                Message::ToggleIncremental,
            )
            .into(),
            Space::with_width(Length::Fill).into(),
            presets::button_nav("Save", Message::SaveConfig, true).into(),
            presets::button_nav("Backup", Message::Backup, true).into(),
        ])
        .spacing(presets::INNER_SPACING)
        .align_items(Alignment::Center);
        Column::with_children(vec![pane_grid.into(), bar.into()])
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING)
            .padding(presets::INNER_SPACING)
            .into()
    }

    fn update(&mut self, message: Message) -> iced::Command<Message> {
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
                    clipboard::write::<Message>(s.to_string());
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
                    clipboard::write::<Message>(s.to_string());
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
                    .set_directory(self.config.get_output())
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
        let content = Scrollable::new(
            self.items.iter().fold(
                Column::new()
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::OUTER_SPACING),
                |content, item| content.push(item.view()),
            ),
        );
        match self.content {
            ConfigPane::Files => presets::pane_border(
                "Files",
                Some(("Open", Message::DialogFolder)),
                content.into(),
            ),
            ConfigPane::Includes => presets::pane_border("Includes", None, content.into()),
            ConfigPane::Excludes => presets::pane_border("Excludes", None, content.into()),
            ConfigPane::Filters => {
                presets::pane_border("Filters", Some(("Add", Message::AddFilter)), content.into())
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
        let row = Row::new()
            .width(Length::Fill)
            // .padding(presets::OUTER_SPACING)
            .spacing(presets::INNER_SPACING)
            .align_items(Alignment::Center);
        let row = match self.state {
            ListState::File => row.push(presets::space_icon()),
            ListState::Folder => row.push(presets::tooltip_right(
                presets::button_icon(">", Message::OpenFolder(self.index), false).into(),
                "Open",
            )),
            ListState::ParentFolder(up) => row.push(presets::tooltip_right(
                presets::button_icon("<", if up { Message::GoUp } else { Message::None }, true)
                    .into(),
                "Go Up",
            )),
            ListState::Include => row.push(presets::tooltip_right(
                presets::button_icon("C", Message::CopyInclude(self.index), false).into(),
                "Copy",
            )),
            ListState::Exclude => row.push(presets::tooltip_right(
                presets::button_icon("C", Message::CopyExclude(self.index), false).into(),
                "Copy",
            )),
            ListState::Error | ListState::Filter => row,
        };
        let row = match &self.state {
            ListState::Error => row.push(presets::text_error(&self.text).width(Length::Fill)),
            ListState::Filter => row,
            _ => row.push(Text::new(&self.text).width(Length::Fill)),
        };
        let row = match &self.state {
            ListState::File | ListState::Folder | ListState::ParentFolder(_) => row
                .push(presets::tooltip_left(
                    presets::button_icon(
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
                presets::button_icon("-", Message::RemoveInclude(self.index), true).into(),
                "Remove",
            )),
            ListState::Exclude => row.push(presets::tooltip_left(
                presets::button_icon("-", Message::RemoveExclude(self.index), true).into(),
                "Remove",
            )),
            ListState::Filter => {
                let i = self.index;
                let mess = move |t| Message::EditFilter(i, t);
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
                    presets::button_icon("-", Message::RemoveFilter(self.index), true).into(),
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

impl ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter> {
    fn crawl_for_files(config: Config) -> Self {
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

    fn backup_files(writer: BackupWriter) -> Self {
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

#[derive(PartialEq, Eq)]
enum ListSort {
    Name,
    Size,
    Time,
}

enum BackupStage {
    Scanning(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Viewing(BackupWriter),
    Performing(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Cancelling(ThreadWrapper<Result<FileInfo, BackupError>, BackupWriter>),
    Failure,
}

struct BackupState {
    config: Config,
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
    fn new(config: Config) -> Self {
        let crawler = ThreadWrapper::crawl_for_files(config.clone());
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

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => match &mut self.stage {
                BackupStage::Scanning(crawler) => {
                    for _ in 0..1000 {
                        match crawler.queue.try_recv() {
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
                                        std::mem::replace(&mut self.stage, BackupStage::Failure)
                                    {
                                        match crawler.handle.join() {
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
                                                self.pagination.change_total(self.total_count);
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
                    for _ in 0..1000 {
                        match wrapper.queue.try_recv() {
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
                                        std::mem::replace(&mut self.stage, BackupStage::Failure)
                                    {
                                        match wrapper.handle.join() {
                                            Ok(bw) => {
                                                self.current_count = 0;
                                                self.stage = BackupStage::Viewing(bw)
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
                BackupStage::Cancelling(_) => {
                    if let BackupStage::Cancelling(wrapper) =
                        std::mem::replace(&mut self.stage, BackupStage::Failure)
                    {
                        std::mem::drop(wrapper.queue);
                        match wrapper.handle.join() {
                            Ok(writer) => {
                                self.current_count = 0;
                                self.stage = BackupStage::Viewing(writer)
                            }
                            Err(_) => self.error.push_str("\nFailure when cancelling the backup"),
                        };
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
            Message::StartBackup => {
                if let BackupStage::Viewing(_) = &self.stage {
                    self.list_sort = ListSort::Name;
                    if let BackupStage::Viewing(mut writer) =
                        std::mem::replace(&mut self.stage, BackupStage::Failure)
                    {
                        writer.list.as_mut().unwrap().sort_unstable();
                        self.stage = BackupStage::Performing(ThreadWrapper::backup_files(writer));
                        self.current_count = 0;
                        self.current_size = 0;
                    }
                }
            }
            Message::CancelBackup => {
                if let BackupStage::Performing(_) = &self.stage {
                    if let BackupStage::Performing(wrapper) =
                        std::mem::replace(&mut self.stage, BackupStage::Failure)
                    {
                        self.stage = BackupStage::Cancelling(wrapper);
                    }
                }
            }
            Message::Export => {
                if let BackupStage::Viewing(writer) = &mut self.stage {
                    if let Some(file) = FileDialog::new()
                        .set_directory(self.config.get_output())
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
            _ => eprintln!("Unexpected GUI message"),
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
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

    fn view(&self) -> Element<Message> {
        let mut scroll = Column::new()
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING);
        match &self.stage {
            BackupStage::Scanning(_) => {
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error[1..])
                            .horizontal_alignment(Horizontal::Left),
                    );
                }
                let brow = Row::with_children(vec![
                    presets::button_nav("Edit", Message::EditConfig, false).into(),
                    Space::with_width(Length::Fill).into(),
                    Text::new(format!(
                        "Scanning for files to backup: {} with total size {}\n",
                        self.total_count,
                        format_size(self.total_size)
                    ))
                    .vertical_alignment(Vertical::Center)
                    .into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav("Backup", Message::None, true).into(),
                ])
                .align_items(Alignment::Center)
                .spacing(presets::INNER_SPACING);
                let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
                Column::with_children(vec![scroll.into(), brow.into()])
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::INNER_SPACING)
                    .into()
            }
            BackupStage::Viewing(writer) => {
                let trow = Row::with_children(vec![
                    presets::button_grey(
                        "Name",
                        Message::SortName,
                        self.list_sort != ListSort::Name,
                    )
                    .width(Length::Fill)
                    .into(),
                    presets::button_grey(
                        "Size",
                        Message::SortSize,
                        self.list_sort != ListSort::Size,
                    )
                    .width(Length::Units(102))
                    .into(),
                    presets::button_grey(
                        "Time",
                        Message::SortTime,
                        self.list_sort != ListSort::Time,
                    )
                    .width(Length::Units(182))
                    .into(),
                ])
                .spacing(presets::INNER_SPACING);
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error[1..])
                            .horizontal_alignment(Horizontal::Left),
                    );
                }
                scroll = self.pagination.push_to(
                    scroll,
                    writer
                        .try_iter_files()
                        .expect("The files should already be crawled at this point"),
                    |f| {
                        Row::with_children(vec![
                            Text::new(f.copy_string()).width(Length::Fill).into(),
                            Text::new(format_size(f.size))
                                .width(Length::Units(102))
                                .horizontal_alignment(Horizontal::Right)
                                .into(),
                            Text::new(f.time.unwrap().format("%Y-%m-%d %H:%M:%S").to_string())
                                .width(Length::Units(182))
                                .horizontal_alignment(Horizontal::Right)
                                .into(),
                            presets::space_scroll().into(),
                        ])
                        .align_items(Alignment::Center)
                        .spacing(presets::INNER_SPACING)
                        .into()
                    },
                );
                let diff = writer.list.as_ref().unwrap().len() - self.total_count as usize;
                let status = if diff > 0 {
                    format!(
                        "{} files with total size {} ({} files have not changed)",
                        self.total_count,
                        format_size(self.total_size),
                        diff
                    )
                } else {
                    format!(
                        "{} files with total size {}",
                        self.total_count,
                        format_size(self.total_size)
                    )
                };
                let brow = Row::with_children(vec![
                    presets::button_nav("Edit", Message::EditConfig, false).into(),
                    Space::with_width(Length::Fill).into(),
                    Text::new(status)
                        .vertical_alignment(Vertical::Center)
                        .into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_color("Export list", Message::Export).into(),
                    presets::button_nav("Backup", Message::StartBackup, true).into(),
                ])
                .align_items(Alignment::Center)
                .spacing(presets::INNER_SPACING);
                let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
                Column::with_children(vec![
                    trow.into(),
                    scroll.into(),
                    presets::space_inner_height().into(),
                    brow.into(),
                ])
                .width(Length::Fill)
                .padding(presets::INNER_SPACING)
                .into()
            }
            BackupStage::Performing(_) | BackupStage::Cancelling(_) => {
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error[1..])
                            .horizontal_alignment(Horizontal::Left),
                    );
                }
                let status = if let BackupStage::Cancelling(_) = self.stage {
                    Text::new("Cancelling the backup...")
                } else if self.current_count >= self.total_count {
                    Text::new("Waiting for the compression to complete...")
                } else {
                    Text::new(&format!(
                        "Backing up file {} of {}, {} of {}",
                        self.current_count,
                        self.total_count,
                        format_size(self.current_size),
                        format_size(self.total_size)
                    ))
                };
                let max = (self.total_size / 1024 + self.total_count as u64) as f32;
                let current = (self.current_size / 1024 + self.current_count as u64) as f32;
                let bar = presets::progress_bar(current + max * 0.005, max * 1.01);
                let brow = Row::with_children(vec![
                    presets::button_nav("Edit", Message::None, false).into(),
                    Space::with_width(Length::Fill).into(),
                    status.vertical_alignment(Vertical::Center).into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav(
                        "Cancel",
                        if let BackupStage::Cancelling(_) = self.stage {
                            Message::None
                        } else {
                            Message::CancelBackup
                        },
                        false,
                    )
                    .into(),
                ])
                .align_items(Alignment::Center)
                .spacing(presets::INNER_SPACING);
                let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
                Column::with_children(vec![scroll.into(), bar.into(), brow.into()])
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::INNER_SPACING)
                    .into()
            }
            BackupStage::Failure => {
                if !self.error.is_empty() {
                    scroll = scroll.push(
                        presets::text_error(&self.error[1..])
                            .horizontal_alignment(Horizontal::Left),
                    );
                }
                let brow = Row::with_children(vec![
                    presets::button_nav("Edit", Message::None, false).into(),
                    Space::with_width(Length::Fill).into(),
                    presets::button_nav("Refresh", Message::Backup, true).into(),
                ])
                .align_items(Alignment::Center)
                .spacing(presets::INNER_SPACING);
                let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
                Column::with_children(vec![scroll.into(), brow.into()])
                    .width(Length::Fill)
                    .spacing(presets::INNER_SPACING)
                    .padding(presets::INNER_SPACING)
                    .into()
            }
        }
    }
}

enum RestoreStage<'a> {
    Error,
    View(BackupReader<'a>, Vec<(bool, String)>, paginated::State),
    // Extract,
}

struct RestoreState<'a> {
    filter: String,
    error: String,
    stage: RestoreStage<'a>,
    all: bool,
}

impl<'a> RestoreState<'a> {
    fn new(mut reader: BackupReader<'a>) -> Self {
        let mut state = Self {
            error: String::new(),
            stage: RestoreStage::Error,
            all: false,
            filter: String::new(),
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
        let size = list.len();
        state.stage = RestoreStage::View(reader, list, paginated::State::new(100, size));
        state
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        //TODO Restore func
        match message {
            Message::ToggleSelected(i) => {
                if let RestoreStage::View(_, list, _) = &mut self.stage {
                    if let Some((b, _)) = list.get_mut(i) {
                        *b = !*b;
                    }
                    self.all = false;
                }
            }
            Message::RestoreAll => todo!(),
            Message::ExtractSelected => todo!(),
            Message::Export => {
                if let RestoreStage::View(reader, _, _) = &mut self.stage {
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
                            self.stage = RestoreStage::Error;
                        }
                    }
                }
            }
            Message::ToggleAll => {
                if let RestoreStage::View(_, list, _) = &mut self.stage {
                    //TODO search filter
                    if self.all {
                        list.iter_mut().for_each(|(b, _)| *b = false);
                        self.all = false;
                    } else {
                        list.iter_mut().for_each(|(b, _)| *b = true);
                        self.all = true;
                    }
                }
            }
            Message::EditFilter(_, s) => {
                self.filter = s;
            }
            Message::AddFilter => {
                //TODO search filter
                todo!();
            }
            Message::GoTo(index) => {
                if let RestoreStage::View(_, _, pagination) = &mut self.stage {
                    pagination.goto(index)
                }
            }
            _ => {}
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let mut scroll = Column::new()
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING);
        if !self.error.is_empty() {
            scroll = scroll.push(presets::text_error(&self.error[1..]))
        }
        let trow = match &self.stage {
            RestoreStage::Error => Space::with_height(Length::Shrink).into(),
            RestoreStage::View(_, list, view) => {
                //TODO search filter
                scroll = view.push_to(scroll, list.iter().enumerate(), |(i, (sel, file))| {
                    Checkbox::new(*sel, file, move |_| Message::ToggleSelected(i))
                        .width(Length::Fill)
                        .into()
                });
                let regex = Regex::new(&self.filter).is_ok();
                Row::with_children(vec![
                    Space::with_width(Length::Units(0)).into(),
                    Checkbox::new(self.all, "", |_| Message::ToggleAll).into(),
                    Space::with_width(Length::Units(presets::LARGE_SPACING)).into(),
                    presets::regex_field(&self.filter, "Regex filter", regex, |s| {
                        Message::EditFilter(0, s)
                    })
                    .width(Length::Fill)
                    .on_submit(Message::AddFilter)
                    .into(),
                    presets::button_nav(
                        "Search",
                        if regex {
                            Message::AddFilter
                        } else {
                            Message::None
                        },
                        true,
                    )
                    .into(),
                ])
                .align_items(Alignment::Center)
                .spacing(presets::INNER_SPACING)
                .into()
            }
        };
        let mut brow = Row::with_children(vec![
            presets::button_nav("Back", Message::Main, false).into(),
            Space::with_width(Length::Fill).into(),
        ])
        .align_items(Alignment::Center)
        .spacing(presets::INNER_SPACING);
        if let RestoreStage::View(reader, list, _) = &self.stage {
            brow = brow
                .push(Text::new(&match reader
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
                .push(presets::button_color(
                    "Extract selected",
                    Message::ExtractSelected,
                ))
                .push(presets::button_color("Restore all", Message::RestoreAll));
        }
        let scroll = presets::scroll_border(scroll.into()).height(Length::Fill);
        Column::with_children(vec![trow, scroll.into(), brow.into()])
            .width(Length::Fill)
            .spacing(presets::INNER_SPACING)
            .padding(presets::INNER_SPACING)
            .into()
    }
}
