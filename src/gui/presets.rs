#![cfg(feature = "gui")]
use std::borrow::Cow;

use iced::alignment::{Horizontal, Vertical};
use iced::pane_grid::Pane;
use iced::pure::widget::{
    button, pane_grid, text_input, Button, Checkbox, Column, Container, PaneGrid, PickList,
    ProgressBar, Row, Scrollable, Space, Text, TextInput, Toggler, Tooltip,
};
use iced::pure::Element;
use iced::{container, progress_bar, tooltip, Alignment, Background, Color, Length, Vector};

use super::Message;

const APP_COLOR: Color = Color::from_rgb(78.0 / 255.0, 155.0 / 255.0, 71.0 / 255.0); //#4E9B47
const APP2_COLOR: Color = Color::from_rgb(172.0 / 255.0, 215.0 / 255.0, 168.0 / 255.0); //#acd7a8
const COMP_COLOR: Color = Color::from_rgb(148.0 / 255.0, 71.0 / 255.0, 155.0 / 255.0); //#94479b
const GREY_COLOR: Color = Color::from_rgb(0.6, 0.6, 0.6);
const LIGHT_COLOR: Color = Color::from_rgb(0.9, 0.9, 0.9);
const DARK_COLOR: Color = Color::from_rgb(0.3, 0.3, 0.3);
const SMALL_RADIUS: f32 = 3.0;
const LARGE_RADIUS: f32 = 5.0;
const ICON_BUTTON_WIDTH: u16 = 30;
const INNER_SPACING: u16 = 3;
pub const OUTER_SPACING: u16 = 6;
pub const LARGE_SPACING: u16 = 6;
const SHADOW_OFFSET: Vector<f32> = Vector::new(1.0, 2.0);
const BORDER_WIDTH: f32 = 2.0;

pub(crate) fn button_color(text: &str, action: Message) -> Button<Message> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label).style(ButtonStyle::ColorButton);
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}
pub(crate) fn button_grey(text: &str, action: Message, light: bool) -> Button<Message> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = if light {
        Button::new(label).style(ButtonStyle::LightButton)
    } else {
        Button::new(label).style(ButtonStyle::GreyButton)
    };
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn button_nav(text: &str, action: Message, forward: bool) -> Button<Message> {
    let label = Text::new(text)
        .width(Length::Units(64))
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label).style(if forward {
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

pub(crate) fn button_icon(text: &str, action: Message, negative: bool) -> Button<Message> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label)
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
    Space::with_width(Length::Shrink)
}

pub(crate) fn space_inner() -> Space {
    Space::with_height(Length::Units(INNER_SPACING))
}

pub(crate) fn space_large() -> Space {
    Space::with_height(Length::Units(LARGE_SPACING))
}

pub(crate) fn button_main(text: &str, alt: bool, action: Message) -> Button<Message> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label)
        .width(Length::Units(200))
        .height(Length::Units(40))
        .style(if alt {
            ButtonStyle::MainButtonAlt
        } else {
            ButtonStyle::MainButton
        });
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn row_list<'a>() -> Row<'a, Message> {
    Row::new()
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(INNER_SPACING)
}

pub(crate) fn row_list2(children: Vec<Element<Message>>) -> Row<Message> {
    Row::with_children(children)
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(INNER_SPACING)
}

pub(crate) fn row_bar(children: Vec<Element<Message>>) -> Row<Message> {
    Row::with_children(children)
        .align_items(Alignment::Center)
        .spacing(INNER_SPACING)
}

pub(crate) fn column_list<'a>() -> Column<'a, Message> {
    Column::new()
        .width(Length::Fill)
        .spacing(INNER_SPACING)
        .padding(INNER_SPACING)
}

pub(crate) fn column_list2(children: Vec<Element<Message>>) -> Column<Message> {
    Column::with_children(children)
        .width(Length::Fill)
        .spacing(INNER_SPACING)
        .padding(INNER_SPACING)
}

pub(crate) fn column_main(children: Vec<Element<Message>>) -> Column<Message> {
    Column::with_children(children)
        .width(Length::Fill)
        .spacing(INNER_SPACING)
        .padding(INNER_SPACING)
}

pub(crate) fn text<S: Into<String>>(text: S) -> Text {
    Text::new(text)
}

pub(crate) fn text_title<S: Into<String>>(text: S) -> Text {
    Text::new(text)
        .size(32)
        .horizontal_alignment(Horizontal::Center)
}

pub(crate) fn text_error<S: Into<String>>(text: S) -> Text {
    Text::new(text)
        .color(COMP_COLOR)
        .horizontal_alignment(Horizontal::Center)
}

pub(crate) fn text_center<S: Into<String>>(text: S) -> Text {
    Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center)
}

pub(crate) fn pane_grid<'a, T, F>(state: &'a pane_grid::State<T>, view: F) -> PaneGrid<Message>
where
    F: Fn(Pane, &'a T) -> pane_grid::Content<'a, Message>,
{
    PaneGrid::new(state, view)
        .on_resize(10, Message::PaneResized)
        .on_drag(Message::PaneDragged)
        .spacing(INNER_SPACING)
}

pub(crate) fn pane_border<'a>(
    title: &str,
    button: Option<(&'a str, Message)>,
    content: Element<'a, Message>,
) -> pane_grid::Content<'a, Message> {
    let title = Row::with_children(vec![
        Space::with_width(Length::Shrink).into(),
        Text::new(title).vertical_alignment(Vertical::Center).into(),
    ])
    .align_items(iced::Alignment::Center)
    .spacing(INNER_SPACING)
    .padding(OUTER_SPACING);
    let mut title_bar = pane_grid::TitleBar::new(title).style(ContainerStyle::PaneTitleBar);
    if let Some((text, action)) = button {
        title_bar = title_bar
            .controls(button_color(text, action))
            .always_show_controls();
    }
    pane_grid::Content::new(content)
        .title_bar(title_bar)
        .style(ContainerStyle::Pane)
}

pub(crate) fn scroll_pane<'a>(
    title: &str,
    button: Option<(&'a str, Message)>,
    content: Element<'a, Message>,
) -> pane_grid::Content<'a, Message> {
    pane_border(title, button, Scrollable::new(content).into())
}

pub(crate) fn scroll_border<'a>(content: Element<'a, Message>) -> Container<'a, Message> {
    Container::new(Scrollable::new(content).height(Length::Fill))
        .style(ContainerStyle::Pane)
        .padding(INNER_SPACING)
}

pub(crate) fn tooltip_right<'a>(content: Element<'a, Message>, tip: &str) -> Tooltip<'a, Message> {
    Tooltip::new(content, tip, tooltip::Position::Right).style(ContainerStyle::Tooltip)
}

pub(crate) fn tooltip_left<'a>(content: Element<'a, Message>, tip: &str) -> Tooltip<'a, Message> {
    Tooltip::new(content, tip, tooltip::Position::Left).style(ContainerStyle::Tooltip)
}

pub(crate) fn regex_field<'a, F>(
    value: &'a String,
    placeholder: &str,
    valid_regex: bool,
    mess: F,
) -> TextInput<'a, Message>
where
    F: 'static + Fn(String) -> Message,
{
    let inp = TextInput::new(placeholder, value, mess).padding(LARGE_SPACING);
    if value.is_empty() {
        inp.style(InputStyle::Normal)
    } else if valid_regex {
        inp.style(InputStyle::Working)
    } else {
        inp.style(InputStyle::Problem)
    }
}

pub(crate) fn progress_bar<'a>(current: f32, max: f32) -> ProgressBar<'a> {
    ProgressBar::new(0.0..=max, current)
        .width(Length::Fill)
        .style(ProgressStyle::Normal)
}

pub(crate) fn toggler<F>(state: bool, label: &str, on_change: F) -> Toggler<Message>
where
    F: 'static + Fn(bool) -> Message,
{
    iced::pure::toggler(Some(label.into()), state, on_change)
        .spacing(INNER_SPACING)
        .text_alignment(Horizontal::Right)
        .style(ToggleStyle::Normal)
        .width(Length::Shrink)
}

pub(crate) fn checkbox<F>(state: bool, label: &str, on_change: F) -> Checkbox<Message>
where
    F: 'static + Fn(bool) -> Message,
{
    Checkbox::new(state, label, on_change)
}

pub(crate) fn pick_list<'a, T, F>(
    options: impl Into<Cow<'a, [T]>>,
    selected: Option<T>,
    on_change: F,
) -> PickList<'a, T, Message>
where
    T: ToString + Eq,
    [T]: ToOwned<Owned = Vec<T>>,
    F: 'static + Fn(T) -> Message,
{
    PickList::new(options, selected, on_change)
        .style(PickListStyle::Normal)
        .width(Length::Shrink)
}

pub enum ButtonStyle {
    GreyButton,
    LightButton,
    MainButton,
    MainButtonAlt,
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

pub enum ToggleStyle {
    Normal,
}

pub enum PickListStyle {
    Normal,
}

impl container::StyleSheet for ContainerStyle {
    fn style(&self) -> container::Style {
        match &self {
            ContainerStyle::PaneTitleBar => container::Style {
                text_color: Some(Color::WHITE),
                background: Some(DARK_COLOR.into()),
                border_radius: SMALL_RADIUS,
                ..Default::default()
            },
            ContainerStyle::Pane => container::Style {
                background: Some(Color::WHITE.into()),
                border_width: BORDER_WIDTH,
                border_color: DARK_COLOR,
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
                background: Some(DARK_COLOR.into()),
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
            ButtonStyle::MainButtonAlt => button::Style {
                background: Some(COMP_COLOR.into()),
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

    fn hovered(&self) -> button::Style {
        button::Style {
            text_color: LIGHT_COLOR,
            shadow_offset: SHADOW_OFFSET,
            ..self.active()
        }
    }
}

impl text_input::StyleSheet for InputStyle {
    fn active(&self) -> text_input::Style {
        text_input::Style {
            background: Color::WHITE.into(),
            border_color: DARK_COLOR,
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
            background: LIGHT_COLOR.into(),
            bar: APP_COLOR.into(),
            border_radius: LARGE_RADIUS,
        }
    }
}

impl iced::toggler::StyleSheet for ToggleStyle {
    fn active(&self, is_active: bool) -> iced::toggler::Style {
        iced::toggler::Style {
            background: if is_active { APP_COLOR } else { GREY_COLOR },
            background_border: None,
            foreground: Color::WHITE,
            foreground_border: None,
        }
    }

    fn hovered(&self, is_active: bool) -> iced::toggler::Style {
        iced::toggler::Style {
            foreground: LIGHT_COLOR,
            ..self.active(is_active)
        }
    }
}

impl iced::pick_list::StyleSheet for PickListStyle {
    fn menu(&self) -> iced::pick_list::Menu {
        iced::pick_list::Menu {
            background: Background::Color(LIGHT_COLOR.into()),
            selected_background: Background::Color(APP_COLOR),
            border_color: DARK_COLOR,
            border_width: 1.0,
            ..iced::pick_list::Menu::default()
        }
    }

    fn active(&self) -> iced::pick_list::Style {
        iced::pick_list::Style {
            border_radius: SMALL_RADIUS,
            background: Background::Color(APP_COLOR.into()),
            text_color: Color::WHITE,
            border_color: APP_COLOR,
            ..iced::pick_list::Style::default()
        }
    }

    fn hovered(&self) -> iced::pick_list::Style {
        iced::pick_list::Style {
            border_color: Color::BLACK,
            text_color: LIGHT_COLOR,
            ..self.active()
        }
    }
}
