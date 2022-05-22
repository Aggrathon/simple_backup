#![cfg(feature = "gui")]
use std::cmp::min;

use iced::pure::widget::{Column, Row, Space};
use iced::pure::Element;
use iced::{Alignment, Length};

use super::{presets, Message};

pub(crate) struct State {
    index: usize,
    total: usize,
    length: usize,
}

impl State {
    pub fn new(length: usize, total: usize) -> Self {
        Self {
            index: 0,
            total,
            length,
        }
    }

    #[allow(dead_code)]
    pub fn next_page(&mut self) {
        if self.index + self.length < self.total {
            self.index += self.length;
        }
    }

    #[allow(dead_code)]
    pub fn prev_page(&mut self) {
        self.index = self.index.saturating_sub(self.length);
    }

    pub fn goto(&mut self, index: usize) {
        self.index = if index > self.total {
            self.total - self.length
        } else {
            index
        };
    }

    pub fn change_total(&mut self, total: usize) {
        self.total = total;
        self.index = 0;
    }

    pub fn push_to<'a, T>(
        &self,
        scroll: Column<'a, Message>,
        items: impl std::iter::Iterator<Item = T>,
        renderer: fn(T) -> Element<'a, Message>,
    ) -> Column<'a, Message> {
        let mut scroll = scroll;
        for item in items.skip(self.index).take(self.length) {
            let item: Element<Message> = renderer(item);
            scroll = scroll.push(item);
        }
        if self.total > self.length {
            scroll = scroll.push(
                Row::with_children(vec![
                    Space::with_width(Length::Fill).into(),
                    presets::button_grey(
                        "<<",
                        if self.index > 0 {
                            Message::GoTo(0)
                        } else {
                            Message::None
                        },
                        false,
                    )
                    .into(),
                    presets::button_grey(
                        "<",
                        if self.index > 0 {
                            Message::GoTo(self.index.saturating_sub(self.length))
                        } else {
                            Message::None
                        },
                        false,
                    )
                    .into(),
                    presets::text_center(&format!(
                        "{:3} - {:3}",
                        self.index,
                        min(self.index + self.length, self.total)
                    ))
                    .into(),
                    presets::button_grey(
                        ">",
                        if self.index + self.length < self.total {
                            Message::GoTo(self.index + self.length)
                        } else {
                            Message::None
                        },
                        false,
                    )
                    .into(),
                    presets::button_grey(
                        ">>",
                        if self.index + self.length < self.total {
                            Message::GoTo(usize::MAX)
                        } else {
                            Message::None
                        },
                        false,
                    )
                    .into(),
                    Space::with_width(Length::Fill).into(),
                ])
                .align_items(Alignment::Center)
                .spacing(presets::INNER_SPACING),
            );
        }
        scroll
    }
}
