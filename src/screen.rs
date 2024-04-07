use ratatui::Frame;

use crate::{message::Message, command::Command};

pub trait Screen {
    fn render(&self, f: &mut Frame<'_>);

    fn update(&mut self, message: Message) -> Option<Command>;
}
