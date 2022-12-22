#![cfg(feature = "gui")]

use std::borrow::Cow;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::pane_grid::Pane;
use iced::widget::{
    pane_grid, tooltip, Button, Checkbox, Column, Container, PaneGrid, PickList, ProgressBar, Row,
    Scrollable, Space, Text, TextInput, Toggler, Tooltip,
};
use iced::{Alignment, Element, Length, Renderer};

use super::{theme, Message};

const ICON_WIDTH_BUTTON: u16 = 30;
const SPACING_INNER: u16 = 3;
const SPACING_OUTER: u16 = 6;
const SPACING_LARGE: u16 = 6;

pub(crate) fn button(text: &str, action: Message) -> Button<Message, Renderer<theme::Theme>> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label);
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}
pub(crate) fn button_grey(
    text: &str,
    action: Message,
    light: bool,
) -> Button<Message, Renderer<theme::Theme>> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = if light {
        Button::new(label).style(theme::Button::LightGrey)
    } else {
        Button::new(label).style(theme::Button::DarkGrey)
    };
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn button_nav(
    text: &str,
    action: Message,
    forward: bool,
) -> Button<Message, Renderer<theme::Theme>> {
    let label = Text::new(text)
        .width(Length::Units(64))
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label).style(if forward {
        theme::Button::Normal
    } else {
        theme::Button::Negative
    });
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn button_icon(
    text: &str,
    action: Message,
    negative: bool,
) -> Button<Message, Renderer<theme::Theme>> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label)
        .style(if negative {
            theme::Button::Negative
        } else {
            theme::Button::Normal
        })
        .width(Length::Units(ICON_WIDTH_BUTTON));
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn space_icon() -> Space {
    Space::with_width(Length::Units(ICON_WIDTH_BUTTON))
}

pub(crate) fn space_scroll() -> Space {
    Space::with_width(Length::Shrink)
}

pub(crate) fn space_inner() -> Space {
    Space::with_height(Length::Units(SPACING_INNER))
}

pub(crate) fn space_large() -> Space {
    Space::with_height(Length::Units(SPACING_LARGE))
}

pub(crate) fn space_hfill() -> Space {
    Space::with_width(Length::Fill)
}

pub(crate) fn button_main(
    text: &str,
    alt: bool,
    action: Message,
) -> Button<Message, Renderer<theme::Theme>> {
    let label = Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center);
    let but = Button::new(label)
        .width(Length::Units(200))
        .height(Length::Units(40))
        .style(if alt {
            theme::Button::MainAlt
        } else {
            theme::Button::Main
        });
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn row_list<'a>() -> Row<'a, Message, Renderer<theme::Theme>> {
    Row::new()
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(SPACING_INNER)
}

pub(crate) fn row_list2(
    children: Vec<Element<Message, Renderer<theme::Theme>>>,
) -> Row<Message, Renderer<theme::Theme>> {
    Row::with_children(children)
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(SPACING_INNER)
}

pub(crate) fn row_bar(
    children: Vec<Element<Message, Renderer<theme::Theme>>>,
) -> Row<Message, Renderer<theme::Theme>> {
    Row::with_children(children)
        .align_items(Alignment::Center)
        .spacing(SPACING_INNER)
}

pub(crate) fn column_list<'a>() -> Column<'a, Message, Renderer<theme::Theme>> {
    Column::new()
        .width(Length::Fill)
        .spacing(SPACING_INNER)
        .padding(SPACING_INNER)
}

pub(crate) fn column_list2(
    children: Vec<Element<Message, Renderer<theme::Theme>>>,
) -> Column<Message, Renderer<theme::Theme>> {
    Column::with_children(children)
        .width(Length::Fill)
        .spacing(SPACING_INNER)
        .padding(SPACING_INNER)
}

pub(crate) fn column_root(
    children: Vec<Element<Message, Renderer<theme::Theme>>>,
) -> Column<Message, Renderer<theme::Theme>> {
    Column::with_children(children)
        .width(Length::Fill)
        .spacing(SPACING_INNER)
        .padding(SPACING_INNER)
}

pub(crate) fn column_main(
    children: Vec<Element<Message, Renderer<theme::Theme>>>,
) -> Column<Message, Renderer<theme::Theme>> {
    Column::with_children(children)
        .align_items(Alignment::Center)
        .spacing(SPACING_LARGE)
        .padding(SPACING_INNER)
}

pub(crate) fn text<'a, S: Into<Cow<'a, str>>>(text: S) -> Text<'a, Renderer<theme::Theme>> {
    Text::new(text)
}

pub(crate) fn text_title<'a, S: Into<Cow<'a, str>>>(text: S) -> Text<'a, Renderer<theme::Theme>> {
    Text::new(text)
        .size(32)
        .horizontal_alignment(Horizontal::Center)
}

pub(crate) fn text_error<'a, S: Into<Cow<'a, str>>>(text: S) -> Text<'a, Renderer<theme::Theme>> {
    Text::new(text).style(theme::Text::Negative)
}

pub(crate) fn text_center<'a, S: Into<Cow<'a, str>>>(text: S) -> Text<'a, Renderer<theme::Theme>> {
    Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .vertical_alignment(Vertical::Center)
        .width(Length::Fill)
}

pub(crate) fn text_center_error<'a, S: Into<Cow<'a, str>>>(
    text: S,
) -> Text<'a, Renderer<theme::Theme>> {
    Text::new(text)
        .horizontal_alignment(Horizontal::Center)
        .style(theme::Text::Negative)
        .vertical_alignment(Vertical::Center)
        .width(Length::Fill)
}

pub(crate) fn pane_grid<'a, T, F>(
    state: &'a pane_grid::State<T>,
    view: F,
) -> PaneGrid<Message, Renderer<theme::Theme>>
where
    F: Fn(Pane, &'a T, bool) -> pane_grid::Content<'a, Message, Renderer<theme::Theme>>,
{
    PaneGrid::new(state, view)
        .on_resize(10, Message::PaneResized)
        .on_drag(Message::PaneDragged)
        .spacing(SPACING_INNER)
}

pub(crate) fn pane_border<'a, S: Into<Cow<'a, str>>>(
    title: S,
    button: Option<(&'a str, Message)>,
    content: Element<'a, Message, Renderer<theme::Theme>>,
) -> pane_grid::Content<'a, Message, Renderer<theme::Theme>> {
    let title = Row::with_children(vec![
        Space::with_width(Length::Shrink).into(),
        Text::new(title).vertical_alignment(Vertical::Center).into(),
    ])
    .align_items(iced::Alignment::Center)
    .spacing(SPACING_INNER)
    .padding(SPACING_OUTER);
    let mut title_bar = pane_grid::TitleBar::new(title).style(theme::Container::PaneTitleBar);
    if let Some((text, action)) = button {
        title_bar = title_bar
            .controls(self::button(text, action))
            .always_show_controls();
    }
    pane_grid::Content::new(content)
        .title_bar(title_bar)
        .style(theme::Container::Pane)
}

pub(crate) fn scroll_pane<'a, S: Into<Cow<'a, str>>>(
    title: S,
    button: Option<(&'a str, Message)>,
    content: Element<'a, Message, Renderer<theme::Theme>>,
) -> pane_grid::Content<'a, Message, Renderer<theme::Theme>> {
    pane_border(title, button, Scrollable::new(content).into())
}

pub(crate) fn scroll_border(
    content: Element<'_, Message, Renderer<theme::Theme>>,
) -> Container<'_, Message, Renderer<theme::Theme>> {
    Container::new(Scrollable::new(content).height(Length::Fill))
        .style(theme::Container::Pane)
        .padding(SPACING_INNER)
        .height(Length::Fill)
}

pub(crate) fn tooltip_right<'a, S: Into<Cow<'a, str>>>(
    content: Element<'a, Message, Renderer<theme::Theme>>,
    tip: S,
) -> Tooltip<'a, Message, Renderer<theme::Theme>> {
    Tooltip::new(content, tip, tooltip::Position::Right).style(theme::Container::Tooltip)
}

pub(crate) fn tooltip_left<'a, S: Into<Cow<'a, str>>>(
    content: Element<'a, Message, Renderer<theme::Theme>>,
    tip: S,
) -> Tooltip<'a, Message, Renderer<theme::Theme>> {
    Tooltip::new(content, tip, tooltip::Position::Left).style(theme::Container::Tooltip)
}

pub(crate) fn regex_field<'a, F>(
    value: &'a String,
    placeholder: &str,
    valid_regex: bool,
    mess: F,
) -> TextInput<'a, Message, Renderer<theme::Theme>>
where
    F: 'static + Fn(String) -> Message,
{
    let inp = TextInput::new(placeholder, value, mess).padding(SPACING_LARGE);
    if value.is_empty() {
        inp.style(theme::TextInput::Normal)
    } else if valid_regex {
        inp.style(theme::TextInput::Working)
    } else {
        inp.style(theme::TextInput::Problem)
    }
}

pub(crate) fn progress_bar(current: f32, max: f32) -> ProgressBar<Renderer<theme::Theme>> {
    ProgressBar::new(0.0..=max, current).width(Length::Fill)
}

pub(crate) fn progress_bar2(current: usize, max: usize) -> ProgressBar<Renderer<theme::Theme>> {
    ProgressBar::new(0.0..=max as f32, current as f32).width(Length::Fill)
}

pub(crate) fn toggler<F>(
    state: bool,
    label: &str,
    on_change: F,
) -> Toggler<Message, Renderer<theme::Theme>>
where
    F: 'static + Fn(bool) -> Message,
{
    iced::widget::toggler(Some(label.into()), state, on_change)
        .spacing(SPACING_INNER)
        .text_alignment(Horizontal::Right)
        .width(Length::Shrink)
}

pub(crate) fn checkbox<F>(
    state: bool,
    label: &str,
    on_change: F,
) -> Checkbox<Message, Renderer<theme::Theme>>
where
    F: 'static + Fn(bool) -> Message,
{
    Checkbox::new(state, label, on_change)
}

pub(crate) fn pick_list<'a, T, F>(
    options: impl Into<Cow<'a, [T]>>,
    selected: Option<T>,
    on_change: F,
) -> PickList<'a, T, Message, Renderer<theme::Theme>>
where
    T: ToString + Eq,
    [T]: ToOwned<Owned = Vec<T>>,
    F: 'static + Fn(T) -> Message,
{
    PickList::new(options, selected, on_change).width(Length::Shrink)
}
