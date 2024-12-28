#![cfg(feature = "gui")]

use std::borrow::Cow;

use iced::alignment::{Horizontal, Vertical};
use iced::font::Weight;
use iced::widget::pane_grid::Pane;
use iced::widget::text::Fragment;
use iced::widget::{
    tooltip, Button, Checkbox, Column, Container, PaneGrid, PickList, ProgressBar, Row, Scrollable,
    Space, Text, TextInput, Toggler, Tooltip,
};
use iced::{Element, Font, Length};

use super::{theme, Message};

const ICON_WIDTH_BUTTON: f32 = 30.0;
const SPACING_INNER: f32 = 3.0;
const SPACING_OUTER: f32 = 6.0;
const SPACING_LARGE: f32 = 6.0;
const TOGGLER_SIZE: f32 = 25.0;
const NAV_BUTTON_WIDTH: f32 = 64.0;
const MAIN_BUTTON_WIDTH: f32 = 200.0;
const MAIN_BUTTON_HEIGHT: f32 = 40.0;

pub(crate) fn button(text: &str, action: Message) -> Element<Message> {
    let label = Text::new(text)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let but = Button::new(label).style(theme::button_normal);
    if let Message::None = action {
        but.into()
    } else {
        but.on_press(action).into()
    }
}
pub(crate) fn button_grey(text: &str, action: Message) -> Button<Message> {
    let label = Text::new(text)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let but = Button::new(label).style(theme::button_grey);
    if let Message::None = action {
        but
    } else {
        but.on_press(action)
    }
}

pub(crate) fn button_group(text: &str, action: Message, selected: bool) -> Button<Message> {
    let label = Text::new(text)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    if selected {
        let mut font = Font::DEFAULT;
        font.weight = Weight::Bold;
        Button::new(label.font(font)).style(theme::button_grey)
    } else if let Message::None = action {
        Button::new(label).style(theme::button_grey)
    } else {
        Button::new(label)
            .style(theme::button_grey)
            .on_press(action)
    }
}

pub(crate) fn button_nav(text: &str, action: Message, forward: bool) -> Element<Message> {
    let label = Text::new(text)
        .width(Length::Fixed(NAV_BUTTON_WIDTH))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let but = Button::new(label).style(if forward {
        theme::button_normal
    } else {
        theme::button_negative
    });
    if let Message::None = action {
        but.into()
    } else {
        but.on_press(action).into()
    }
}

pub(crate) fn button_icon(text: &str, action: Message, negative: bool) -> Element<Message> {
    let label = Text::new(text)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let but = Button::new(label)
        .style(if negative {
            theme::button_negative
        } else {
            theme::button_normal
        })
        .width(Length::Fixed(ICON_WIDTH_BUTTON));
    if let Message::None = action {
        but.into()
    } else {
        but.on_press(action).into()
    }
}

pub(crate) fn space_icon<'a>() -> Element<'a, Message> {
    Space::with_width(Length::Fixed(ICON_WIDTH_BUTTON)).into()
}

pub(crate) fn space_scroll<'a>() -> Element<'a, Message> {
    Space::with_width(Length::Shrink).into()
}

pub(crate) fn space_inner<'a>() -> Element<'a, Message> {
    Space::with_height(Length::Fixed(SPACING_INNER)).into()
}

pub(crate) fn space_large<'a>() -> Element<'a, Message> {
    Space::with_height(Length::Fixed(SPACING_LARGE)).into()
}

pub(crate) fn space_hfill<'a>() -> Element<'a, Message> {
    Space::with_width(Length::Fill).into()
}

pub(crate) fn button_main(text: &str, alt: bool, action: Message) -> Element<Message> {
    let label = Text::new(text)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let but = Button::new(label)
        .width(Length::Fixed(MAIN_BUTTON_WIDTH))
        .height(Length::Fixed(MAIN_BUTTON_HEIGHT))
        .style(if alt {
            theme::button_negative
        } else {
            theme::button_normal
        });
    if let Message::None = action {
        but.into()
    } else {
        but.on_press(action).into()
    }
}

pub(crate) fn row_list<'a>() -> Row<'a, Message> {
    Row::new()
        .width(Length::Fill)
        .align_y(Vertical::Center)
        .spacing(SPACING_INNER)
}

pub(crate) fn row_list2(children: Vec<Element<Message>>) -> Row<Message> {
    Row::with_children(children)
        .width(Length::Fill)
        .align_y(Vertical::Center)
        .spacing(SPACING_INNER)
}

pub(crate) fn row_bar(children: Vec<Element<Message>>) -> Row<Message> {
    Row::with_children(children)
        .align_y(Vertical::Center)
        .spacing(SPACING_INNER)
}

pub(crate) fn column_list<'a>() -> Column<'a, Message> {
    Column::new()
        .width(Length::Fill)
        .spacing(SPACING_INNER)
        .padding(SPACING_INNER)
}

pub(crate) fn column_list2(children: Vec<Element<Message>>) -> Column<Message> {
    Column::with_children(children)
        .width(Length::Fill)
        .spacing(SPACING_INNER)
        .padding(SPACING_INNER)
}

pub(crate) fn column_root(children: Vec<Element<Message>>) -> Column<Message> {
    Column::with_children(children)
        .width(Length::Fill)
        .spacing(SPACING_INNER)
        .padding(SPACING_INNER)
}

pub(crate) fn column_main(column: Column<Message>) -> Column<Message> {
    column
        .align_x(Horizontal::Center)
        .spacing(SPACING_LARGE)
        .padding(SPACING_INNER)
}

pub(crate) fn text<'a, S: Into<Fragment<'a>>>(text: S) -> Text<'a> {
    Text::new(text.into())
}

pub(crate) fn text_vcenter<'a, S: Into<Fragment<'a>>>(text: S) -> Element<'a, Message> {
    Text::new(text.into()).align_y(Vertical::Center).into()
}

pub(crate) fn text_title<'a, S: Into<Fragment<'a>>>(text: S) -> Element<'a, Message> {
    Text::new(text.into()).size(32).center().into()
}

pub(crate) fn text_error<'a, S: Into<Fragment<'a>>>(text: S) -> Text<'a> {
    Text::new(text.into()).style(iced::widget::text::danger)
}

pub(crate) fn text_center<'a, S: Into<Fragment<'a>>>(text: S) -> Element<'a, Message> {
    Text::new(text.into())
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .into()
}

pub(crate) fn text_center_error<'a, S: Into<Fragment<'a>>>(text: S) -> Element<'a, Message> {
    Text::new(text.into())
        .align_x(Horizontal::Center)
        .style(iced::widget::text::danger)
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .into()
}

pub(crate) fn pane_grid<T, F>(
    state: &iced::widget::pane_grid::State<T>,
    view: F,
) -> PaneGrid<Message>
where
    F: Fn(Pane, &T, bool) -> iced::widget::pane_grid::Content<Message>,
{
    PaneGrid::new(state, view)
        .on_resize(10, Message::PaneResized)
        .on_drag(Message::PaneDragged)
        .spacing(SPACING_INNER)
}

pub(crate) fn pane_border<'a, S: Into<Fragment<'a>>>(
    title: S,
    button: Option<(&'a str, Message)>,
    content: Element<'a, Message>,
) -> iced::widget::pane_grid::Content<'a, Message> {
    let title = Row::with_children(vec![
        Space::with_width(Length::Shrink).into(),
        Text::new(title.into()).align_y(Vertical::Center).into(),
    ])
    .align_y(Vertical::Center)
    .spacing(SPACING_INNER)
    .padding(SPACING_OUTER);
    let mut title_bar = iced::widget::pane_grid::TitleBar::new(title).style(theme::container_title);
    if let Some((text, action)) = button {
        title_bar = title_bar
            .controls(self::button(text, action))
            .always_show_controls();
    }
    iced::widget::pane_grid::Content::new(content)
        .title_bar(title_bar)
        .style(theme::container_pane)
}

pub(crate) fn scroll_pane<'a, S: Into<Cow<'a, str>>>(
    title: S,
    button: Option<(&'a str, Message)>,
    content: Element<'a, Message>,
) -> iced::widget::pane_grid::Content<'a, Message> {
    pane_border(
        title,
        button,
        Scrollable::new(content).style(theme::scrollbar).into(),
    )
}

pub(crate) fn scroll_border(content: Element<'_, Message>) -> Element<'_, Message> {
    Container::new(
        Scrollable::new(content)
            .height(Length::Fill)
            .style(theme::scrollbar),
    )
    .style(theme::container_pane)
    .padding(SPACING_INNER)
    .height(Length::Fill)
    .into()
}

pub(crate) fn tooltip_right<'a, S: Into<Fragment<'a>>>(
    content: Element<'a, Message>,
    tip: S,
) -> Tooltip<'a, Message> {
    let tip = text(tip);
    Tooltip::new(content, tip, tooltip::Position::Right).style(theme::tooltip)
}

pub(crate) fn tooltip_left<'a, S: Into<Fragment<'a>>>(
    content: Element<'a, Message>,
    tip: S,
) -> Tooltip<'a, Message> {
    let tip = text(tip);
    Tooltip::new(content, tip, tooltip::Position::Left).style(theme::tooltip)
}

pub(crate) fn regex_field<'a, F>(
    value: &'a str,
    placeholder: &str,
    valid_regex: bool,
    mess: F,
) -> TextInput<'a, Message>
where
    F: 'static + Fn(String) -> Message,
{
    let inp = TextInput::new(placeholder, value)
        .padding(SPACING_LARGE)
        .on_input(mess);
    if value.is_empty() {
        inp.style(theme::input_primary)
    } else if valid_regex {
        inp.style(theme::input_success)
    } else {
        inp.style(theme::input_danger)
    }
}

pub(crate) fn progress_bar<'a>(current: f32, max: f32) -> ProgressBar<'a> {
    ProgressBar::new(0.0..=max, current)
        .width(Length::Fill)
        .style(theme::progressbar)
}

pub(crate) fn progress_bar2<'a>(current: usize, max: usize) -> ProgressBar<'a> {
    ProgressBar::new(0.0..=max as f32, current as f32)
        .width(Length::Fill)
        .style(theme::progressbar)
}

pub(crate) fn toggler<F>(state: bool, label: &str, on_change: F) -> Element<Message>
where
    F: 'static + Fn(bool) -> Message,
{
    _toggler(state, label, on_change)
        .style(theme::toggle)
        .into()
}

pub(crate) fn toggler_comp<F>(state: bool, label: &str, on_change: F) -> Element<Message>
where
    F: 'static + Fn(bool) -> Message,
{
    _toggler(state, label, on_change)
        .style(theme::toggle_comp)
        .into()
}

fn _toggler<F>(state: bool, label: &str, on_change: F) -> Toggler<Message>
where
    F: 'static + Fn(bool) -> Message,
{
    iced::widget::toggler(state)
        .label(label)
        .on_toggle(on_change)
        .spacing(SPACING_INNER)
        .text_alignment(Horizontal::Right)
        .width(Length::Shrink)
        .size(TOGGLER_SIZE)
}

pub(crate) fn checkbox<F>(state: bool, label: &str, on_change: F) -> Checkbox<Message>
where
    F: 'static + Fn(bool) -> Message,
{
    Checkbox::new(label, state)
        .on_toggle(on_change)
        .style(theme::checkbox_color)
}

pub(crate) fn pick_list<T, F>(
    options: &'_ [T],
    selected: Option<T>,
    on_change: F,
) -> Element<Message>
where
    T: ToString + Eq + Clone,
    F: 'static + Fn(T) -> Message,
{
    PickList::new(options, selected, on_change)
        .width(Length::Shrink)
        .style(theme::dropdown)
        .into()
}
