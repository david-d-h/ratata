use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventListenerError {
    #[error("failed to read from event stream: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("failed to send acquired event to the bound receiver: {0}")]
    SendError(#[from] mpsc::SendError<Event>),
}

pub type JoinHandle = thread::JoinHandle<Result<(), EventListenerError>>;

pub fn listen(timeout: Duration) -> (JoinHandle, Receiver<Event>, Arc<AtomicBool>) {
    let (tx, rx) = mpsc::channel();

    let quit_handle = Arc::new(AtomicBool::new(false));

    let should_quit = quit_handle.clone();

    let handle = thread::spawn(move || loop {
        if should_quit.load(Ordering::Relaxed) {
            break Ok(());
        }

        if !event::poll(timeout)? {
            continue;
        }

        let event = event::read()?;

        // Filter out the KeyEventKind::Release and KeyEventKind::Repeat presses.
        if !matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press) {
            continue;
        }

        tx.send(event)?;
    });

    (handle, rx, quit_handle)
}
