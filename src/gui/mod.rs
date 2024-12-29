#![cfg(feature = "gui")]
/// This module contains the logic for running the program through a GUI
use iced::widget::{column, pane_grid, row, Space};
use iced::{Element, Length, Subscription};
use rfd::{FileDialog, MessageDialog};
use theme::theme;

use self::backup::BackupState;
use self::config::ConfigState;
use self::merge::MergeState;
use self::restore::RestoreState;
use crate::backup::{BackupReader, BACKUP_FILE_EXTENSION, CONFIG_FILE_EXTENSION};
use crate::config::Config;
use crate::utils::{default_dir, get_config_from_path};

mod backup;
mod config;
mod merge;
mod paginated;
mod presets;
mod restore;
mod theme;
mod threads;

#[allow(dead_code)]
#[cfg_attr(target_os = "windows", link(name = "Kernel32"))]
extern "system" {
    fn FreeConsole() -> i32;
}

pub fn gui(_hide_terminal: bool) {
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    if _hide_terminal {
        unsafe {
            // Safety: Windows syscall to hide the console
            FreeConsole()
        };
    }
    #[cfg(windows)]
    let bytes = include_bytes!("..\\..\\target\\icon.bytes").to_vec();
    #[cfg(not(windows))]
    let bytes = include_bytes!("../../target/icon.bytes").to_vec();
    let icon = iced::window::icon::from_rgba(bytes, 64, 64).expect("Could not load icon");
    let settings = iced::window::settings::Settings {
        icon: Some(icon),
        ..Default::default()
    };
    iced::application(title, update, view)
        .theme(theme)
        .window(settings)
        .subscription(subscription)
        .run()
        .expect("Failed to run application");
}

enum ApplicationState {
    Main(MainState),
    Config(ConfigState),
    Backup(BackupState),
    Merge(MergeState),
    Restore(RestoreState),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum Message {
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    MainView,
    CreateConfig,
    EditConfig,
    BackupView,
    RestoreView,
    MergeView,
    Incremental(bool),
    ThreadCount(u32),
    CompressionQuality(i32),
    IncludeAdd(usize),
    IncludeRemove(usize),
    IncludeOpen(usize),
    ExcludeAdd(usize),
    ExcludeRemove(usize),
    ExcludeOpen(usize),
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
    Merge,
    Flat(bool),
    All(bool),
    Delete(bool),
    Repeat,
    None,
}

impl Default for ApplicationState {
    fn default() -> Self {
        ApplicationState::Main(MainState::new())
    }
}

fn title(state: &ApplicationState) -> String {
    match state {
        ApplicationState::Main(_) => String::from("simple_backup"),
        ApplicationState::Config(_) => String::from("simple_backup - Config"),
        ApplicationState::Backup(_) => String::from("simple_backup - Backup"),
        ApplicationState::Merge(_) => String::from("simple_backup - Merge"),
        ApplicationState::Restore(_) => String::from("simple_backup - Restore"),
    }
}

fn update(state: &mut ApplicationState, message: Message) {
    match message {
        Message::CreateConfig => *state = ApplicationState::Config(ConfigState::new(true, true)),
        Message::EditConfig => {
            if let ApplicationState::Backup(state2) = state {
                *state =
                    ApplicationState::Config(ConfigState::from(std::mem::take(&mut state2.config)))
            } else if let Some(config) = open_config() {
                *state = ApplicationState::Config(ConfigState::from(config))
            }
        }
        Message::BackupView => {
            if let ApplicationState::Config(state2) = state {
                let mut config = std::mem::take(&mut state2.config);
                if let Some(path) = FileDialog::new()
                    .set_directory(config.get_output(true))
                    .set_title("Where should the backups be stored")
                    .pick_folder()
                {
                    config.output = path;
                    *state = ApplicationState::Backup(BackupState::new(config))
                }
            } else if let Some(config) = open_config() {
                *state = ApplicationState::Backup(BackupState::new(config))
            };
        }
        Message::RestoreView => {
            if let Some(reader) = open_backup() {
                *state = ApplicationState::Restore(restore::RestoreState::new(reader));
            }
        }
        Message::None => {
            eprintln!("Unspecified GUI message");
        }
        Message::MainView => {
            *state = ApplicationState::Main(MainState::new());
        }
        Message::MergeView => {
            *state = ApplicationState::Merge(MergeState::new());
        }
        _ => match state {
            ApplicationState::Main(_) => {}
            ApplicationState::Config(state) => state.update(message),
            ApplicationState::Backup(state) => state.update(message),
            ApplicationState::Merge(state) => state.update(message),
            ApplicationState::Restore(state) => state.update(message),
        },
    }
}

fn view(state: &ApplicationState) -> Element<Message> {
    match state {
        ApplicationState::Main(state) => state.view(),
        ApplicationState::Config(state) => state.view(),
        ApplicationState::Backup(state) => state.view(),
        ApplicationState::Merge(state) => state.view(),
        ApplicationState::Restore(state) => state.view(),
    }
}

fn open_config() -> Option<Config> {
    FileDialog::new()
        .set_directory(default_dir())
        .set_title("Open existing config or backup file")
        .add_filter("Config and backup files", &[
            &CONFIG_FILE_EXTENSION[1..],
            &BACKUP_FILE_EXTENSION[1..],
        ])
        .add_filter("Config files", &[&CONFIG_FILE_EXTENSION[1..]])
        .add_filter("Backup files", &[&BACKUP_FILE_EXTENSION[1..]])
        .pick_file()
        .and_then(|file| match get_config_from_path(file) {
            Ok(config) => Some(config),
            Err(e) => {
                MessageDialog::new()
                    .set_description(e.to_string())
                    .set_level(rfd::MessageLevel::Error)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .set_title("Problem with reading config")
                    .show();
                None
            }
        })
}
fn open_backup() -> Option<BackupReader> {
    FileDialog::new()
        .set_directory(default_dir())
        .set_title("Open backup file")
        .add_filter("Backup files", &[&BACKUP_FILE_EXTENSION[1..]])
        .pick_file()
        .map(BackupReader::new)
}

fn subscription(state: &ApplicationState) -> iced::Subscription<Message> {
    match state {
        ApplicationState::Backup(state) => state.subscription(),
        ApplicationState::Merge(state) => state.subscription(),
        ApplicationState::Restore(state) => state.subscription(),
        _ => Subscription::none(),
    }
}

struct MainState {}

impl MainState {
    fn new() -> Self {
        Self {}
    }

    fn view(&self) -> Element<Message> {
        let column = presets::column_main(column![
            Space::with_height(Length::Fill),
            presets::text_title("simple_backup"),
            Space::with_height(Length::Shrink),
            presets::button_main("Create", false, Message::CreateConfig),
            presets::button_main("Edit", false, Message::EditConfig),
            presets::button_main("Backup", false, Message::BackupView),
            presets::button_main("Merge", true, Message::MergeView),
            presets::button_main("Restore", true, Message::RestoreView),
            Space::with_height(Length::Fill),
        ]);
        row![
            Space::with_width(Length::Fill),
            column,
            Space::with_width(Length::Fill),
        ]
        .into()
    }
}
