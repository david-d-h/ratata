use crossterm::event::{Event, KeyCode, KeyEventState, KeyModifiers, MouseEvent};

pub type KeyState = KeyEventState;

pub struct KeyMsg {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub state: KeyState,
}

pub type MouseMsg = MouseEvent;

pub enum Message {
    Key(KeyMsg),
    Mouse(MouseMsg),
    Resize(u16, u16),
    FocusGained,
    FocusLost,
    #[cfg(feature = "paste")]
    Paste(String),
    Shutdown,
    Tick,
}

impl From<Event> for Message {
    fn from(value: Event) -> Self {
        match value {
            Event::FocusGained => Message::FocusGained,
            Event::FocusLost => Message::FocusLost,
            Event::Key(key) => Message::Key(KeyMsg {
                code: key.code,
                modifiers: key.modifiers,
                state: key.state,
            }),
            Event::Mouse(mouse) => Message::Mouse(mouse),
            #[cfg(feature = "paste")]
            Event::Paste(value) => Message::Paste(value),
            Event::Resize(x, y) => Message::Resize(x, y),
        }
    }
}