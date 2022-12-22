#![cfg(feature = "gui")]

use iced::widget::{
    button, checkbox, container, pane_grid, pick_list, progress_bar, scrollable, text, text_input,
    toggler,
};
use iced::{application, overlay, Background, Color, Vector};

const COLOR_APP: Color = Color::from_rgb(78.0 / 255.0, 155.0 / 255.0, 71.0 / 255.0); //#4E9B47
const COLOR_APP_LIGHT: Color = Color::from_rgb(172.0 / 255.0, 215.0 / 255.0, 168.0 / 255.0); //#acd7a8
const COLOR_COMP: Color = Color::from_rgb(148.0 / 255.0, 71.0 / 255.0, 155.0 / 255.0); //#94479b
const COLOR_GREY: Color = Color::from_rgb(0.6, 0.6, 0.6);
const COLOR_LIGHT: Color = Color::from_rgb(0.9, 0.9, 0.9);
const COLOR_DARK: Color = Color::from_rgb(0.3, 0.3, 0.3);
const COLOR_BACKGROUND: Color = Color::WHITE;
const RADIUS_SMALL: f32 = 3.0;
const RADIUS_LARGE: f32 = 5.0;
const SHADOW_OFFSET: Vector<f32> = Vector::new(1.0, 2.0);
const BORDER_WIDTH: f32 = 2.0;
const BORDER_SMALL: f32 = 1.0;

#[derive(Default)]
pub struct Theme {}

impl application::StyleSheet for Theme {
    type Style = ();

    fn appearance(&self, _style: &Self::Style) -> application::Appearance {
        application::Appearance {
            background_color: COLOR_BACKGROUND,
            text_color: Color::BLACK,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum Text {
    #[default]
    Normal,
    // Color,
    Negative,
}

impl text::StyleSheet for Theme {
    type Style = Text;

    fn appearance(&self, style: Self::Style) -> text::Appearance {
        match style {
            Text::Normal => text::Appearance { color: None },
            // Text::Color => text::Appearance {
            //     color: Some(COLOR_APP),
            // },
            Text::Negative => text::Appearance {
                color: Some(COLOR_COMP),
            },
        }
    }
}

#[derive(Default)]
pub enum Button {
    #[default]
    Normal,
    Negative,
    DarkGrey,
    LightGrey,
    Main,
    MainAlt,
}

impl button::StyleSheet for Theme {
    type Style = Button;

    fn active(&self, style: &Self::Style) -> button::Appearance {
        match style {
            Button::DarkGrey => button::Appearance {
                background: Some(COLOR_DARK.into()),
                text_color: COLOR_BACKGROUND,
                border_radius: RADIUS_SMALL,
                ..Default::default()
            },
            Button::LightGrey => button::Appearance {
                background: Some(COLOR_GREY.into()),
                text_color: COLOR_BACKGROUND,
                border_radius: RADIUS_SMALL,
                ..Default::default()
            },
            Button::Main => button::Appearance {
                background: Some(COLOR_APP.into()),
                text_color: COLOR_BACKGROUND,
                border_radius: RADIUS_LARGE,
                ..Default::default()
            },
            Button::MainAlt => button::Appearance {
                background: Some(COLOR_COMP.into()),
                text_color: COLOR_BACKGROUND,
                border_radius: RADIUS_LARGE,
                ..Default::default()
            },
            Button::Normal => button::Appearance {
                background: Some(COLOR_APP.into()),
                text_color: COLOR_BACKGROUND,
                border_radius: RADIUS_SMALL,
                ..Default::default()
            },
            Button::Negative => button::Appearance {
                background: Some(COLOR_COMP.into()),
                text_color: COLOR_BACKGROUND,
                border_radius: RADIUS_SMALL,
                ..Default::default()
            },
        }
    }

    fn hovered(&self, style: &Self::Style) -> button::Appearance {
        button::Appearance {
            text_color: COLOR_LIGHT,
            shadow_offset: SHADOW_OFFSET,
            ..self.active(style)
        }
    }
}

#[derive(Default)]
pub enum Container {
    PaneTitleBar,
    #[default]
    Pane,
    Tooltip,
}

impl container::StyleSheet for Theme {
    type Style = Container;

    fn appearance(&self, style: &Self::Style) -> container::Appearance {
        match style {
            Container::PaneTitleBar => container::Appearance {
                text_color: Some(COLOR_BACKGROUND),
                background: Some(COLOR_DARK.into()),
                border_radius: RADIUS_SMALL,
                ..Default::default()
            },
            Container::Pane => container::Appearance {
                background: Some(COLOR_BACKGROUND.into()),
                border_width: BORDER_WIDTH,
                border_color: COLOR_DARK,
                border_radius: RADIUS_SMALL,
                ..Default::default()
            },
            Container::Tooltip => container::Appearance {
                background: Some(COLOR_LIGHT.into()),
                border_radius: RADIUS_SMALL,
                ..container::Appearance::default()
            },
        }
    }
}

#[derive(Default)]
pub enum TextInput {
    #[default]
    Normal,
    Working,
    Problem,
}

impl text_input::StyleSheet for Theme {
    type Style = TextInput;

    fn active(&self, _style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: COLOR_BACKGROUND.into(),
            border_color: COLOR_DARK,
            border_radius: RADIUS_SMALL,
            border_width: BORDER_SMALL,
            // ..Default::default()
        }
    }

    fn focused(&self, style: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            ..self.active(style)
        }
    }

    fn placeholder_color(&self, _style: &Self::Style) -> Color {
        COLOR_LIGHT
    }

    fn value_color(&self, style: &Self::Style) -> Color {
        match style {
            TextInput::Normal => COLOR_DARK,
            TextInput::Working => COLOR_DARK,
            TextInput::Problem => COLOR_COMP,
        }
    }

    fn selection_color(&self, _style: &Self::Style) -> Color {
        COLOR_APP_LIGHT
    }
}

#[derive(Default)]
pub enum ProgressBar {
    #[default]
    Normal,
}

impl progress_bar::StyleSheet for Theme {
    type Style = ProgressBar;

    fn appearance(&self, _style: &Self::Style) -> progress_bar::Appearance {
        progress_bar::Appearance {
            background: COLOR_LIGHT.into(),
            bar: COLOR_APP.into(),
            border_radius: RADIUS_LARGE,
        }
    }
}

#[derive(Default)]
pub enum Toggle {
    #[default]
    Normal,
}

impl toggler::StyleSheet for Theme {
    type Style = Toggle;

    fn active(&self, _style: &Self::Style, is_active: bool) -> toggler::Appearance {
        toggler::Appearance {
            background: if is_active { COLOR_APP } else { COLOR_GREY },
            background_border: None,
            foreground: COLOR_BACKGROUND,
            foreground_border: None,
        }
    }

    fn hovered(&self, style: &Self::Style, is_active: bool) -> toggler::Appearance {
        toggler::Appearance {
            foreground: COLOR_LIGHT,
            ..self.active(style, is_active)
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum PickList {
    #[default]
    Normal,
}

impl pick_list::StyleSheet for Theme {
    type Style = PickList;

    fn active(&self, _style: &<Self as pick_list::StyleSheet>::Style) -> pick_list::Appearance {
        pick_list::Appearance {
            border_radius: RADIUS_SMALL,
            background: Background::Color(COLOR_APP),
            text_color: COLOR_BACKGROUND,
            border_color: COLOR_APP,
            border_width: BORDER_WIDTH,
            icon_size: 0.7,
            placeholder_color: COLOR_DARK,
        }
    }

    fn hovered(&self, style: &<Self as pick_list::StyleSheet>::Style) -> pick_list::Appearance {
        pick_list::Appearance {
            border_color: Color::BLACK,
            text_color: COLOR_LIGHT,
            ..self.active(style)
        }
    }
}

impl overlay::menu::StyleSheet for Theme {
    type Style = PickList;

    fn appearance(&self, _style: &Self::Style) -> overlay::menu::Appearance {
        overlay::menu::Appearance {
            text_color: Color::BLACK,
            background: Background::Color(COLOR_LIGHT),
            border_width: BORDER_SMALL,
            border_radius: RADIUS_SMALL,
            border_color: COLOR_DARK,
            selected_text_color: COLOR_BACKGROUND,
            selected_background: Background::Color(COLOR_APP),
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum Scrollable {
    #[default]
    Normal,
}

impl scrollable::StyleSheet for Theme {
    type Style = Scrollable;

    fn active(&self, _style: &Self::Style) -> scrollable::Scrollbar {
        scrollable::Scrollbar {
            background: None,
            border_radius: RADIUS_SMALL,
            border_width: BORDER_SMALL,
            border_color: Color::TRANSPARENT,
            scroller: scrollable::Scroller {
                color: COLOR_APP,
                border_radius: RADIUS_SMALL,
                border_width: BORDER_SMALL,
                border_color: COLOR_APP,
            },
        }
    }

    fn hovered(&self, style: &Self::Style) -> scrollable::Scrollbar {
        scrollable::Scrollbar {
            scroller: scrollable::Scroller {
                color: COLOR_APP_LIGHT,
                border_radius: RADIUS_SMALL,
                border_width: BORDER_SMALL,
                border_color: COLOR_APP_LIGHT,
            },
            ..self.active(style)
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum PaneGrid {
    #[default]
    Normal,
}

impl pane_grid::StyleSheet for Theme {
    type Style = PaneGrid;

    fn picked_split(&self, _style: &Self::Style) -> Option<pane_grid::Line> {
        None
    }

    fn hovered_split(&self, _style: &Self::Style) -> Option<pane_grid::Line> {
        None
    }
}

#[derive(Default, Clone, Copy)]
pub enum Checkbox {
    #[default]
    Normal,
}

impl checkbox::StyleSheet for Theme {
    type Style = Checkbox;

    fn active(&self, _style: &Self::Style, _is_checked: bool) -> checkbox::Appearance {
        checkbox::Appearance {
            background: Background::Color(COLOR_BACKGROUND),
            checkmark_color: COLOR_APP,
            border_radius: RADIUS_SMALL,
            border_width: BORDER_SMALL,
            border_color: COLOR_DARK,
            text_color: None,
        }
    }

    fn hovered(&self, style: &Self::Style, is_checked: bool) -> checkbox::Appearance {
        checkbox::Appearance {
            background: Background::Color(if is_checked {
                COLOR_APP
            } else {
                COLOR_APP_LIGHT
            }),
            checkmark_color: COLOR_BACKGROUND,
            // border_color: COLOR_APP,
            ..self.active(style, is_checked)
        }
    }
}
