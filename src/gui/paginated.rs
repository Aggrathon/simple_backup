#![cfg(feature = "gui")]

use std::cmp::min;

use iced::widget::{Column, Space};
use iced::{Element, Length};

use super::{presets, Message};

pub(crate) struct State {
    pub index: usize,
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

    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        self.index = 0;
    }

    pub fn get_total(&self) -> usize {
        self.total
    }

    pub fn push_to<'a, T>(
        &self,
        scroll: Column<'a, Message>,
        items: impl std::iter::Iterator<Item = T>,
        renderer: fn(T) -> Element<'a, Message>,
    ) -> Column<'a, Message> {
        let mut scroll = scroll;
        let count = min(self.index + self.length, self.total) - self.index;
        for item in items.skip(self.index).take(count) {
            let item: Element<Message> = renderer(item);
            scroll = scroll.push(item);
        }
        if self.total > self.length {
            scroll = scroll.push(presets::row_list2(vec![
                Space::with_width(Length::Fill).into(),
                presets::button_grey(
                    "<<",
                    if self.index > 0 {
                        Message::GoTo(0)
                    } else {
                        Message::None
                    },
                )
                .into(),
                presets::button_grey(
                    "<",
                    if self.index > 0 {
                        Message::GoTo(self.index.saturating_sub(self.length))
                    } else {
                        Message::None
                    },
                )
                .into(),
                presets::text_vcenter(format!(
                    "{} - {} ({})",
                    self.index,
                    min(self.index + self.length, self.total),
                    self.total
                )),
                presets::button_grey(
                    ">",
                    if self.index + self.length < self.total {
                        Message::GoTo(self.index + self.length)
                    } else {
                        Message::None
                    },
                )
                .into(),
                presets::button_grey(
                    ">>",
                    if self.index + self.length < self.total {
                        Message::GoTo(usize::MAX)
                    } else {
                        Message::None
                    },
                )
                .into(),
                Space::with_width(Length::Fill).into(),
            ]));
        }
        scroll
    }
}
