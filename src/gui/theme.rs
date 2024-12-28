#![cfg(feature = "gui")]

use iced::theme::palette::{Background, Danger, Extended, Primary, Secondary, Success};
use iced::theme::Palette;
use iced::widget::{
    button, checkbox, container, pick_list, progress_bar, scrollable, text_input, toggler,
};
use iced::{border, Border, Color, Shadow, Theme, Vector};

use super::ApplicationState;

const COLOR_APP: Color = Color::from_rgb(78.0 / 255.0, 155.0 / 255.0, 71.0 / 255.0); //#4E9B47
const COLOR_COMP: Color = Color::from_rgb(148.0 / 255.0, 71.0 / 255.0, 155.0 / 255.0); //#94479b
const COLOR_GREY: Color = Color::from_rgb(0.6, 0.6, 0.6);
const COLOR_LIGHT: Color = Color::from_rgb(0.9, 0.9, 0.9);
const RADIUS_SMALL: f32 = 4.0;
const RADIUS_LARGE: f32 = 8.0;
const SHADOW_OFFSET: Vector<f32> = Vector::new(1.3, 2.0);
const BORDER_WIDTH: f32 = 3.0;
const BORDER_SMALL: f32 = 1.5;

pub fn theme(_state: &ApplicationState) -> Theme {
    Theme::custom_with_fn(
        "white_green_pruple".to_string(),
        Palette {
            background: Color::WHITE,
            text: Color::BLACK,
            primary: COLOR_LIGHT,
            success: COLOR_APP,
            danger: COLOR_COMP,
        },
        |p| Extended {
            background: Background::new(p.background, p.text),
            primary: Primary::generate(p.success, p.background, p.text),
            secondary: Secondary::generate(COLOR_GREY, p.text),
            success: Success::generate(p.success, p.background, p.text),
            danger: Danger::generate(p.danger, p.background, p.text),
            is_dark: false,
        },
    )
}

pub fn button_normal(theme: &Theme, status: button::Status) -> button::Style {
    let pal = theme.palette();
    let ext = theme.extended_palette();
    match status {
        button::Status::Active => button::Style {
            background: Some(ext.success.base.color.into()),
            text_color: pal.background,
            border: border::rounded(RADIUS_SMALL),
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(ext.success.strong.color.into()),
            text_color: pal.background,
            border: border::rounded(RADIUS_SMALL),
            shadow: Shadow {
                offset: SHADOW_OFFSET,
                color: ext.success.weak.color,
                ..Default::default()
            },
        },
        button::Status::Pressed => button::Style {
            background: Some(ext.success.strong.color.into()),
            text_color: ext.success.strong.text,
            border: border::rounded(RADIUS_SMALL),
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(pal.primary.into()),
            text_color: pal.text,
            border: border::rounded(RADIUS_SMALL),
            ..Default::default()
        },
    }
}

pub fn button_negative(theme: &Theme, status: button::Status) -> button::Style {
    let pal = theme.palette();
    let ext = theme.extended_palette();
    match status {
        button::Status::Active => button::Style {
            background: Some(ext.danger.base.color.into()),
            text_color: pal.background,
            border: border::rounded(RADIUS_SMALL),
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(ext.danger.strong.color.into()),
            text_color: pal.background,
            border: border::rounded(RADIUS_SMALL),
            shadow: Shadow {
                offset: SHADOW_OFFSET,
                color: ext.danger.weak.color,
                ..Default::default()
            },
        },
        button::Status::Pressed => button::Style {
            background: Some(ext.danger.strong.color.into()),
            text_color: ext.danger.strong.text,
            border: border::rounded(RADIUS_SMALL),
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(pal.primary.into()),
            text_color: pal.text,
            border: border::rounded(RADIUS_SMALL),
            ..Default::default()
        },
    }
}

pub fn button_grey(theme: &Theme, status: button::Status) -> button::Style {
    let pal = theme.palette();
    let ext = theme.extended_palette();
    button::Style {
        background: Some(
            match status {
                button::Status::Active => ext.secondary.base.color,
                button::Status::Hovered => ext.secondary.weak.color,
                button::Status::Pressed => ext.secondary.strong.color,
                button::Status::Disabled => pal.primary,
            }
            .into(),
        ),
        text_color: match status {
            button::Status::Active => pal.background,
            button::Status::Hovered => pal.background,
            button::Status::Pressed => pal.background,
            button::Status::Disabled => pal.text,
        },
        border: Border {
            radius: RADIUS_SMALL.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn input_primary(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.value = theme.palette().text;
    style
}

pub fn input_success(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.value = theme.palette().success;
    style
}

pub fn input_danger(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.value = theme.palette().danger;
    style
}

pub fn container_pane(theme: &Theme) -> container::Style {
    container::transparent(theme).border(
        border::color(theme.palette().primary)
            .rounded(RADIUS_LARGE)
            .width(BORDER_WIDTH),
    )
}

pub fn container_title(theme: &Theme) -> container::Style {
    container::background(theme.palette().primary).border(border::rounded(RADIUS_LARGE))
}

pub fn tooltip(theme: &Theme) -> container::Style {
    container::background(theme.palette().primary).border(border::rounded(RADIUS_SMALL))
}

pub fn progressbar(theme: &Theme) -> progress_bar::Style {
    progress_bar::success(theme)
}

pub fn toggle(theme: &Theme, status: toggler::Status) -> toggler::Style {
    _toggle(theme, status, theme.palette().success)
}

pub fn toggle_comp(theme: &Theme, status: toggler::Status) -> toggler::Style {
    _toggle(theme, status, theme.palette().danger)
}

fn _toggle(theme: &Theme, status: toggler::Status, color: Color) -> toggler::Style {
    let palette = theme.extended_palette();
    let mut style = toggler::default(theme, status);
    match status {
        toggler::Status::Active { is_toggled } | toggler::Status::Hovered { is_toggled } => {
            if is_toggled {
                style.background = color;
                style.foreground = palette.background.base.color;
            }
        }
        _ => {}
    }
    style.foreground_border_width = BORDER_SMALL;
    style
}

pub fn checkbox_color(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    match status {
        checkbox::Status::Active { is_checked }
        | checkbox::Status::Hovered { is_checked }
        | checkbox::Status::Disabled { is_checked } => {
            if is_checked {
                checkbox::success(theme, status)
            } else {
                let mut style = checkbox::danger(theme, status);
                style.text_color = Some(theme.palette().danger);
                style
            }
        }
    }
}

pub fn dropdown(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let palette = theme.extended_palette();
    pick_list::Style {
        text_color: palette.background.base.text,
        background: palette.background.base.color.into(),
        placeholder_color: palette.background.strong.color,
        handle_color: palette.background.weak.text,
        border: match status {
            pick_list::Status::Active => Border {
                radius: RADIUS_SMALL.into(),
                width: BORDER_SMALL,
                color: palette.background.base.text,
            },
            pick_list::Status::Hovered | pick_list::Status::Opened => Border {
                radius: RADIUS_SMALL.into(),
                width: BORDER_SMALL * 1.5,
                color: palette.primary.base.color,
            },
        },
    }
}

pub fn scrollbar(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    style.horizontal_rail.background = None;
    style.vertical_rail.background = None;
    style
}
