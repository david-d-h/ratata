pub mod screen;
pub mod events;
pub mod command;
pub mod message;
pub mod application;

pub use ratatui;

pub mod prelude {
    pub use crossterm::event::{KeyCode, KeyModifiers};

    pub use ratatui::Frame;
    pub use ratatui::backend::CrosstermBackend;

    pub use crate::application::Builder as Application;
    pub use crate::message::{Message, KeyMsg, MouseMsg, KeyState};
    pub use crate::command::{self, Command};
    pub use crate::screen::Screen;
}
