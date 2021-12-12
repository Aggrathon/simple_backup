#![cfg(feature = "gui")]
/// This module contains the logic for running the program through a GUI
use crate::{
    config::Config,
    files::{FileCrawler, FileInfo},
    utils::get_config_from_pathbuf,
};
use iced::{
    button, executor, pane_grid, pick_list, scrollable, Align, Application, Checkbox, Column,
    Command, Element, Length, PaneGrid, PickList, Row, Scrollable, Settings, Space, Text,
};
use rfd::{FileDialog, MessageDialog};

pub fn gui() {
    ApplicationState::run(Settings::default()).unwrap();
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
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
    AddInclude(usize),
    RemoveInclude(usize),
    CopyInclude(usize),
    AddExclude(usize),
    RemoveExclude(usize),
    CopyExclude(usize),
    AddFilter,
    RemoveFilter(usize),
    EditFilter(usize),
    OpenFolder(usize),
    GoUp,
    DialogFolder,
    None,
}

enum ApplicationState {
    Main(MainState),
    Config(ConfigState),
    Backup(BackupState),
    Restore(RestoreState),
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
            ApplicationState::Config(_) => String::from("simple_backup"),
            ApplicationState::Backup(_) => String::from("simple_backup"),
            ApplicationState::Restore(_) => String::from("simple_backup"),
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
                if let Some(file) = FileDialog::new()
                    .set_directory(dirs::home_dir().unwrap_or_default())
                    .set_title("Open existing config or backup file")
                    .add_filter("Config and backup files", &["yml", "tar.zst"])
                    .add_filter("Config files", &["yml"])
                    .add_filter("Backup files", &["tar.zst"])
                    .pick_file()
                {
                    match get_config_from_pathbuf(file) {
                        Ok(config) => *self = ApplicationState::Config(ConfigState::from(config)),
                        Err(e) => {
                            MessageDialog::new()
                                .set_description(&e.to_string())
                                .set_level(rfd::MessageLevel::Error)
                                .set_buttons(rfd::MessageButtons::Ok)
                                .set_title("Problem with reading config")
                                .show();
                        }
                    }
                }
                Command::none()
            }
            Message::Backup => {
                *self = ApplicationState::Backup(BackupState {});
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
                ApplicationState::Backup(_) => todo!(),
                ApplicationState::Restore(_) => todo!(),
            },
        }
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        match self {
            ApplicationState::Main(state) => state.view(),
            ApplicationState::Config(state) => state.view(),
            ApplicationState::Backup(_) => todo!(),
            ApplicationState::Restore(_) => todo!(),
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
            presets::button_main(&mut self.edit, "Edit", Message::EditConfig)
                // .on_press(Message::EditConfig)
                .into(),
            presets::button_main(&mut self.backup, "Backup", Message::None)
                // .on_press(Message::Backup)
                .into(),
            presets::button_main(&mut self.config, "Restore", Message::None)
                // .on_press(Message::Restore)
                .into(),
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
            presets::button_color(&mut self.back, "Back", Message::Main).into(),
            Space::with_width(Length::Fill).into(),
            PickList::new(
                &mut self.threads,
                &self.thread_alt,
                Some(self.config.threads),
                Message::ThreadCount,
            )
            .into(),
            Text::new(if self.config.threads > 1 {
                " Threads "
            } else {
                " Thread  "
            })
            .into(),
            Space::with_width(Length::Units(10)).into(),
            Checkbox::new(
                self.config.incremental,
                "Incremental Backups",
                Message::ToggleIncremental,
            )
            .into(),
            Space::with_width(Length::Fill).into(),
            presets::button_color(&mut self.save, "Save", Message::None).into(),
            presets::button_color(&mut self.backup, "Backup", Message::None).into(),
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
            Message::AddInclude(i) => {
                let pane = self.panes.get_mut(&self.files).unwrap();
                if let Some(li) = pane.items.get_mut(i) {
                    let s = std::mem::replace(&mut li.text, String::new());
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
                    let s = std::mem::replace(&mut li.text, String::new());
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
            Message::AddFilter => todo!(),
            Message::RemoveFilter(i) => {
                if i < self.config.regex.len() {
                    self.config.regex.remove(i);
                    self.refresh_filters();
                    self.refresh_files();
                }
            }
            Message::EditFilter(i) => todo!("Edit filter {}", i),
            Message::OpenFolder(i) => {
                let pane = self.panes.get_mut(&self.files).unwrap();
                if let Some(li) = pane.items.get_mut(i) {
                    self.current_dir =
                        FileInfo::from(std::mem::replace(&mut li.text, String::new()));
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
                    ListState::ParentFolder,
                    self.current_dir.get_string().to_string(),
                    if self.current_dir.get_path().parent().is_some() {
                        Message::GoUp
                    } else {
                        Message::None
                    },
                    if parent {
                        Message::None
                    } else {
                        Message::AddInclude(0)
                    },
                    if parent {
                        Message::AddExclude(0)
                    } else {
                        Message::None
                    },
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
            Err(e) => pane.items.push(ListItem::new(
                ListState::Error,
                format!("{}", e),
                Message::None,
                Message::None,
                Message::None,
            )),
        };
    }

    fn refresh_includes(&mut self) {
        let pane = self.panes.get_mut(&self.includes).unwrap();
        pane.items.clear();
        pane.items
            .extend(self.config.include.iter().enumerate().map(|(i, s)| {
                ListItem::new(
                    ListState::CopyItem,
                    s.to_string(),
                    Message::CopyInclude(i),
                    Message::None,
                    Message::RemoveInclude(i),
                )
            }));
    }

    fn refresh_excludes(&mut self) {
        let pane = self.panes.get_mut(&self.excludes).unwrap();
        pane.items.clear();
        pane.items
            .extend(self.config.exclude.iter().enumerate().map(|(i, s)| {
                ListItem::new(
                    ListState::CopyItem,
                    s.to_string(),
                    Message::CopyExclude(i),
                    Message::None,
                    Message::RemoveExclude(i),
                )
            }));
    }

    fn refresh_filters(&mut self) {
        let pane = self.panes.get_mut(&self.filters).unwrap();
        pane.items.clear();
        pane.items
            .extend(self.config.regex.iter().enumerate().map(|(i, s)| {
                ListItem::new(
                    ListState::EditItem,
                    s.to_string(),
                    Message::EditFilter(i),
                    Message::None,
                    Message::RemoveFilter(i),
                )
            }));
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
            .spacing(presets::OUTER_SPACING);
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
    ParentFolder,
    CopyItem,
    EditItem,
    Error,
}

struct ListItem {
    state: ListState,
    open_state: button::State,
    open_action: Message,
    text: String,
    add_state: button::State,
    add_action: Message,
    remove_state: button::State,
    remove_action: Message,
}

impl ListItem {
    fn new(
        state: ListState,
        text: String,
        open_action: Message,
        add_action: Message,
        remove_action: Message,
    ) -> Self {
        Self {
            state,
            text,
            open_action,
            add_action,
            remove_action,
            open_state: button::State::new(),
            add_state: button::State::new(),
            remove_state: button::State::new(),
        }
    }

    fn error(text: String) -> Self {
        Self::new(
            ListState::Error,
            text,
            Message::None,
            Message::None,
            Message::None,
        )
    }

    fn file(text: String, included: bool, is_dir: bool, index: usize) -> Self {
        let open = if is_dir {
            Message::OpenFolder(index)
        } else {
            Message::None
        };
        if included {
            Self::new(
                ListState::File,
                text,
                open,
                Message::None,
                Message::AddExclude(index),
            )
        } else {
            Self::new(
                ListState::File,
                text,
                open,
                Message::AddInclude(index),
                Message::None,
            )
        }
    }

    fn view(&mut self) -> Element<Message> {
        let row = Row::new()
            .width(Length::Fill)
            .padding(presets::OUTER_SPACING)
            .spacing(presets::INNER_SPACING)
            .align_items(Align::Center);
        let row = match self.state {
            ListState::File => {
                if let Message::None = self.open_action {
                    row.push(presets::space_icon())
                } else {
                    row.push(presets::tooltip_right(
                        presets::button_icon(&mut self.open_state, ">", self.open_action, false)
                            .into(),
                        "Open",
                    ))
                }
            }
            ListState::ParentFolder => row.push(presets::tooltip_right(
                presets::button_icon(&mut self.open_state, "<", self.open_action, true).into(),
                "Go Up",
            )),
            ListState::CopyItem => {
                if let Message::None = self.open_action {
                    row
                } else {
                    row.push(presets::tooltip_right(
                        presets::button_icon(&mut self.open_state, "C", self.open_action, false)
                            .into(),
                        "Copy",
                    ))
                }
            }
            ListState::Error => row,
            ListState::EditItem => row.push(presets::tooltip_right(
                presets::button_icon(&mut self.open_state, "E", self.open_action, false).into(),
                "Edit",
            )),
        };
        let row = if let ListState::Error = self.state {
            row.push(presets::text_error(&self.text).width(Length::Fill))
        } else {
            row.push(Text::new(&self.text).width(Length::Fill))
        };
        let row = match self.state {
            ListState::File | ListState::ParentFolder => row
                .push(presets::tooltip_left(
                    presets::button_icon(&mut self.add_state, "+", self.add_action, false).into(),
                    "Include",
                ))
                .push(presets::tooltip_left(
                    presets::button_icon(&mut self.remove_state, "-", self.remove_action, true)
                        .into(),
                    "Exclude",
                )),
            ListState::CopyItem | ListState::EditItem => row.push(presets::tooltip_left(
                presets::button_icon(&mut self.remove_state, "-", self.remove_action, true).into(),
                "Remove",
            )),
            ListState::Error => row,
        };
        let row = row.push(presets::space_scroll());
        row.into()
    }
}

struct BackupState {}

struct RestoreState {}

mod presets {
    use iced::{
        button, container, pane_grid, tooltip, Button, Color, Element, Length, Row, Space, Text,
        Tooltip,
    };

    use super::Message;

    const APP_COLOR: Color = Color::from_rgb(78.0 / 255.0, 155.0 / 255.0, 71.0 / 255.0); //#4E9B47
    const COMP_COLOR: Color = Color::from_rgb(148.0 / 255.0, 71.0 / 255.0, 155.0 / 255.0); //#94479b
    const GREY_COLOR: Color = Color::from_rgb(0.65, 0.65, 0.65);
    const LIGHT_COLOR: Color = Color::from_rgb(0.9, 0.9, 0.9);
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
        let mut title_bar = pane_grid::TitleBar::new(title).style(Container::PaneTitleBar);
        if let Some((text, state, action)) = button {
            title_bar = title_bar
                .controls(button_color(state, text, action))
                .always_show_controls();
        }
        pane_grid::Content::new(content)
            .title_bar(title_bar)
            .style(Container::Pane)
    }

    pub(crate) fn tooltip_right<'a>(
        content: Element<'a, Message>,
        tip: &str,
    ) -> Tooltip<'a, Message> {
        Tooltip::new(content, tip, tooltip::Position::Right).style(Container::Tooltip)
    }

    pub(crate) fn tooltip_left<'a>(
        content: Element<'a, Message>,
        tip: &str,
    ) -> Tooltip<'a, Message> {
        Tooltip::new(content, tip, tooltip::Position::Left).style(Container::Tooltip)
    }

    pub enum ButtonStyle {
        MainButton,
        ColorButton,
        NegativeButton,
    }

    pub enum Container {
        PaneTitleBar,
        Pane,
        Tooltip,
    }

    impl container::StyleSheet for Container {
        fn style(&self) -> container::Style {
            match &self {
                Container::PaneTitleBar => container::Style {
                    text_color: Some(Color::WHITE),
                    background: Some(GREY_COLOR.into()),
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                Container::Pane => container::Style {
                    background: Some(Color::WHITE.into()),
                    border_width: 2.0,
                    border_color: GREY_COLOR,
                    border_radius: SMALL_RADIUS,
                    ..Default::default()
                },
                Container::Tooltip => container::Style {
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
}
