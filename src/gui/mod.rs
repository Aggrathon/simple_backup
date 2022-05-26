#![cfg(feature = "gui")]
/// This module contains the logic for running the program through a GUI
use iced::pure::widget::{pane_grid, Column, Row, Space};
use iced::pure::{Application, Element};
use iced::{executor, Alignment, Command, Length, Settings, Subscription};
use rfd::{FileDialog, MessageDialog};

use self::backup::BackupState;
use self::config::ConfigState;
use self::restore::RestoreState;
use crate::backup::BackupReader;
use crate::config::Config;
use crate::utils::get_config_from_pathbuf;

mod backup;
mod config;
mod paginated;
mod presets;
mod restore;

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
    MainView,
    CreateConfig,
    EditConfig,
    BackupView,
    RestoreView,
    Incremental(bool),
    ThreadCount(u32),
    CompressionQuality(i32),
    IncludeAdd(usize),
    IncludeRemove(usize),
    IncludeCopy(usize),
    ExcludeAdd(usize),
    ExcludeRemove(usize),
    ExcludeCopy(usize),
    FilterAdd,
    FilterRemove(usize),
    FilterEdit(usize, String),
    FolderOpen(usize),
    FolderUp,
    FolderDialog,
    Save,
    SortName,
    SortSize,
    SortTime,
    GoTo(usize),
    Backup,
    Cancel,
    Export,
    Tick,
    Toggle(usize),
    ToggleAll,
    Restore,
    Extract,
    Flat(bool),
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
            Message::BackupView => {
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
            Message::RestoreView => {
                if let Some(reader) = open_backup() {
                    *self = ApplicationState::Restore(restore::RestoreState::new(reader));
                }
                Command::none()
            }
            Message::None => {
                eprintln!("Unspecified GUI message");
                Command::none()
            }
            Message::MainView => {
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
            presets::button_main("Backup", false, Message::BackupView).into(),
            presets::button_main("Restore", true, Message::RestoreView).into(),
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
