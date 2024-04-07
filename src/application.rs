use std::io::Write;
use std::any::TypeId;
use std::collections::HashMap;
use std::{io, time, mem, thread};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};

use crossterm::event::Event;

use ratatui::backend::Backend;

use thiserror::Error;

use crate::{
    events,
    message::Message,
    command::Command,
    screen::Screen,
};

#[derive(Debug, Error)]
#[error("the event source was disconnected")]
pub struct EventSourceDisconnectedError;

#[derive(Debug, Error)]
#[error("could not find a registered screen for: {:?}", 0)]
pub struct MissingScreenError(TypeId);

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    EventSourceDisconnected(#[from] EventSourceDisconnectedError),
    #[error(transparent)]
    MissingScreen(#[from] MissingScreenError),
    #[error("failed to execute a crossterm command: {0}")]
    CrosstermCommandExecution(io::Error),
    #[error("failed to enable or disable raw mode: {0}")]
    RawMode(io::Error),
}

type ScreenEntry = (TypeId, Box<dyn Screen>);

pub struct Application<B: Backend> {
    startup_callback: Option<fn() -> Command>,
    shutdown_callback: Option<fn() -> Command>,
    terminal: ratatui::Terminal<B>,
    sink: Box<dyn Write>,
    tick_rate: time::Duration,
    last_tick: Option<time::Instant>,
    event_poll_rate: time::Duration,
    screens: HashMap<TypeId, Box<dyn Screen>>,
    active_screen_entry: Option<ScreenEntry>,
    previous_screen_entry: Option<ScreenEntry>,
    exiting: bool,
}

impl<B: Backend> Application<B> {
    #[inline(always)]
    pub fn builder() -> Builder {
        Builder::new()
    }

    fn try_read_event(&self, events: &Receiver<Event>) -> Result<Option<Event>, EventSourceDisconnectedError> {
        events.try_recv().map_or_else(
            |err| match err {
                mpsc::TryRecvError::Disconnected => Err(EventSourceDisconnectedError),
                mpsc::TryRecvError::Empty => Ok(None),
            },
            |event| Ok(Some(event)),
        )
    }

    fn shutdown_screens(&mut self) {
        self.screens.values_mut().for_each(|s| {
            let _ = s.update(Message::Shutdown);
        });
    }

    fn get_screen(&mut self, screen: TypeId) -> Result<ScreenEntry, MissingScreenError> {
        self.screens.remove_entry(&screen).map_or_else(|| Err(MissingScreenError(screen)), Ok)
    }

    fn activate_screen(&mut self, screen: TypeId) -> Result<(), MissingScreenError> {
        let new = self.get_screen(screen)?;

        let previous = mem::replace(&mut self.active_screen_entry, Some(new));

        let replaced = mem::replace(&mut self.previous_screen_entry, previous);

        if let Some((ident, screen)) = replaced {
            assert!(self.screens.insert(ident, screen).is_none());
        }

        Ok(())
    }

    fn handle_command(&mut self, command: Command) -> Result<(), RuntimeError> {
        match command {
            | Command::Batch(commands) => {
                for command in commands {
                    self.handle_command(command)?;
                }

                Ok(())
            },
            | Command::EnableRawMode => crossterm::terminal::enable_raw_mode().map_err(RuntimeError::RawMode),
            | Command::DisableRawMode => crossterm::terminal::disable_raw_mode().map_err(RuntimeError::RawMode),
            | Command::Screen(ident) => Ok(self.activate_screen(ident)?),
            | Command::Crossterm(command) =>
                crossterm::execute!(self.sink, command).map_err(RuntimeError::CrosstermCommandExecution),
            | Command::Quit => {
                self.exiting = true;
                Ok(())
            },
        }
    }

    pub fn run<S: Screen + 'static>(mut self) -> Result<(), RuntimeError> {
        let screen = TypeId::of::<S>();

        if let Some(callback) = self.startup_callback {
            self.handle_command(callback())?;
        }

        self.activate_screen(screen)?;

        let (_, events, event_quit_handle) = events::listen(self.event_poll_rate);

        loop {
            if self.exiting {
                break self.shutdown_screens();
            }

            if let Some(last_tick) = &self.last_tick {
                thread::sleep(self.tick_rate.saturating_sub(last_tick.elapsed()));
            }

            self.last_tick = Some(time::Instant::now());

            let message = match self.try_read_event(&events)? {
                Some(event) => Message::from(event),
                None => Message::Tick,
            };

            let screen = &mut self.active_screen_entry.as_mut().unwrap().1;

            if let Some(command) = screen.update(message) {
                self.handle_command(command)?;
            }

            let screen = &mut self.active_screen_entry.as_mut().unwrap().1;

            let _ = self.terminal.draw(|f| screen.render(f)).unwrap();
        }

        if let Some(callback) = self.shutdown_callback {
            self.handle_command(callback())?;
        }

        event_quit_handle.store(true, Ordering::Relaxed);

        Ok(())
    }
}

#[derive(Default)]
pub struct Builder {
    event_poll_rate: Option<time::Duration>,
    screens: HashMap<TypeId, Box<dyn Screen>>,
    tick_rate: Option<time::Duration>,
    startup_callback: Option<fn() -> Command>,
    shutdown_callback: Option<fn() -> Command>,
}

impl Builder {
    #[inline(always)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn event_polling_rate(mut self, rate: time::Duration) -> Self {
        self.event_poll_rate = Some(rate);
        self
    }

    pub fn screen<S: Screen + 'static>(mut self, screen: S) -> Self {
        self.screens.insert(TypeId::of::<S>(), Box::new(screen));
        self
    }

    pub fn tick_rate(mut self, rate: time::Duration) -> Self {
        self.tick_rate = Some(rate);
        self
    }

    pub fn frames_per_second(mut self, fps: u8) -> Self {
        self.tick_rate = Some(time::Duration::from_secs_f32(1. / fps as f32));
        self
    }

    pub fn on_startup(mut self, callback: fn() -> Command) -> Self {
        self.startup_callback = Some(callback);
        self
    }

    pub fn on_shutdown(mut self, callback: fn() -> Command) -> Self {
        self.shutdown_callback = Some(callback);
        self
    }

    pub fn build<W, B>(self, sink: W, backend: B) -> Result<Application<B>, io::Error>
        where W: Write + 'static, B: Backend,
    {
        let tick_rate = self.tick_rate.unwrap_or(time::Duration::from_secs_f32(1. / 30.));

        let event_poll_rate = self.event_poll_rate.unwrap_or(tick_rate / 2);

        let terminal = ratatui::Terminal::new(backend)?;

        Ok(Application {
            shutdown_callback: self.shutdown_callback,
            startup_callback: self.startup_callback,
            terminal,
            sink: Box::new(sink),
            last_tick: None,
            tick_rate,
            event_poll_rate,
            screens: self.screens,
            exiting: false,
            previous_screen_entry: None,
            active_screen_entry: None,
        })
    }
}