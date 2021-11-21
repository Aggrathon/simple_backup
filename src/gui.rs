#![cfg(feature = "gui")]
use std::{borrow::Cow, cmp::max};

/// This module *will contain* the logic for running the program through a GUI
use crate::config::Config;
use iced::{
    button, executor, pane_grid, pick_list, scrollable, text_input, Align, Application, Button,
    Checkbox, Column, Command, Element, Length, PaneGrid, PickList, Row, Scrollable, Settings,
    Space, Text, TextInput,
};

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
        _clipboard: &mut iced::Clipboard,
    ) -> iced::Command<Self::Message> {
        match message {
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                if let ApplicationState::Config(state) = self {
                    state.panes.resize(&split, ratio);
                }
            }
            Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                if let ApplicationState::Config(state) = self {
                    state.panes.swap(&pane, &target);
                }
            }
            Message::PaneDragged(_) => {}
            Message::CreateConfig => *self = ApplicationState::Config(ConfigState::new()),
            Message::EditConfig => todo!(),
            Message::Backup => todo!(),
            Message::Restore => todo!(),
            Message::None => eprintln!("Unspecified GUI message"),
            Message::Main => *self = ApplicationState::Main(MainState::new()),
            Message::ToggleIncremental(t) => {
                if let ApplicationState::Config(state) = self {
                    state.config.incremental = t;
                }
            }
            Message::ThreadCount(text) => {
                if let ApplicationState::Config(state) = self {
                    state.config.set_threads(text);
                }
            }
        }
        Command::none()
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
            presets::button_main(&mut self.edit, "Edit", Message::None)
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
        .spacing(10);

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
}

impl ConfigState {
    fn new() -> Self {
        let (mut panes, files) = pane_grid::State::new(Pane::new(ConfigPane::Files));
        let (incs, _) = panes
            .split(
                pane_grid::Axis::Vertical,
                &files,
                Pane::new(ConfigPane::Includes),
            )
            .unwrap();
        let (excs, _) = panes
            .split(
                pane_grid::Axis::Horizontal,
                &incs,
                Pane::new(ConfigPane::Excludes),
            )
            .unwrap();
        let (filts, _) = panes
            .split(
                pane_grid::Axis::Horizontal,
                &excs,
                Pane::new(ConfigPane::Filters),
            )
            .unwrap();
        Self {
            config: Config::new(),
            panes,
            back: button::State::new(),
            save: button::State::new(),
            backup: button::State::new(),
            threads: pick_list::State::default(),
            thread_alt: (1u32..num_cpus::get() as u32 + 1).collect(),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let pane_grid = PaneGrid::new(&mut self.panes, |id, pane| pane.content())
            .on_resize(10, Message::PaneResized)
            .on_drag(Message::PaneDragged)
            .spacing(5);
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
        .spacing(5)
        .align_items(Align::Center);
        Column::with_children(vec![pane_grid.into(), bar.into()])
            .width(Length::Fill)
            .spacing(2)
            .padding(2)
            .into()
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
}

impl Pane {
    fn new(content: ConfigPane) -> Self {
        Self {
            content,
            scroll: scrollable::State::new(),
            top_button: button::State::new(),
        }
    }

    fn content(&mut self) -> pane_grid::Content<Message> {
        let content = Scrollable::new(&mut self.scroll)
            .width(Length::Fill)
            .spacing(10);
        // let content = self
        //     .items
        //     .iter_mut()
        //     .fold(content, |content, item| content.push(item.row()));
        match self.content {
            ConfigPane::Files => presets::pane_border(
                "Files",
                "Open",
                &mut self.top_button,
                Message::None,
                content.into(),
            ),
            ConfigPane::Includes => presets::pane_border(
                "Includes",
                "Add",
                &mut self.top_button,
                Message::None,
                content.into(),
            ),
            ConfigPane::Excludes => presets::pane_border(
                "Excludes",
                "Add",
                &mut self.top_button,
                Message::None,
                content.into(),
            ),
            ConfigPane::Filters => presets::pane_border(
                "Filters",
                "Add",
                &mut self.top_button,
                Message::None,
                content.into(),
            ),
        }
    }
}

struct BackupState {}

struct RestoreState {}

mod presets {
    use iced::{button, container, pane_grid, Button, Color, Element, Length, Row, Space, Text};

    use super::Message;

    const APP_COLOR: Color = Color::from_rgb(78.0 / 255.0, 155.0 / 255.0, 71.0 / 255.0);
    const GREY_COLOR: Color = Color::from_rgb(0.65, 0.65, 0.65);
    const SMALL_RADIUS: f32 = 3.0;
    const LARGE_RADIUS: f32 = 5.0;

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

    pub(crate) fn pane_border<'a>(
        title: &str,
        button: &str,
        button_state: &'a mut button::State,
        button_action: Message,
        content: Element<'a, Message>,
    ) -> pane_grid::Content<'a, Message> {
        let title = Row::with_children(vec![
            Space::with_width(Length::Shrink).into(),
            Text::new(title).into(),
            Space::with_width(Length::Fill).into(),
            button_color(button_state, button, button_action).into(),
        ])
        .align_items(iced::Align::Center)
        .spacing(5)
        .padding(2);
        let title_bar = pane_grid::TitleBar::new(title).style(Container::PaneTitleBar);

        pane_grid::Content::new(content)
            .title_bar(title_bar)
            .style(Container::Pane)
    }

    pub enum ButtonStyle {
        MainButton,
        ColorButton,
        Button,
    }

    pub enum Container {
        PaneTitleBar,
        Pane,
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
            }
        }
    }

    impl button::StyleSheet for ButtonStyle {
        fn active(&self) -> button::Style {
            match &self {
                ButtonStyle::Button => button::Style {
                    background: Some(APP_COLOR.into()),
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
            }
        }
    }
}
