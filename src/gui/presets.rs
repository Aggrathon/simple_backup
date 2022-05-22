#![cfg(feature = "gui")]
use iced::alignment::{Horizontal, Vertical};
use iced::pure::widget::{
    button, pane_grid, text_input, Button, Container, Row, Scrollable, Text, TextInput, Tooltip,
};
use iced::pure::Element;
use iced::{container, progress_bar, tooltip, Color, Length, ProgressBar, Space};

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
    Space::with_width(Length::Units(0))
}

pub(crate) fn space_inner_height() -> Space {
    Space::with_height(Length::Units(INNER_SPACING))
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

pub(crate) fn text_title(text: &str) -> Text {
    Text::new(text)
        .size(32)
        .horizontal_alignment(Horizontal::Center)
}

pub(crate) fn text_error(text: &str) -> Text {
    Text::new(text)
        .color(COMP_COLOR)
        .horizontal_alignment(Horizontal::Center)
}

pub(crate) fn text_center(text: &str) -> Text {
    Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center)
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
            background: LIGHT_COLOR.into(),
            bar: APP_COLOR.into(),
            border_radius: LARGE_RADIUS,
        }
    }
}
