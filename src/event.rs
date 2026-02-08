use std::time::Duration;

use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent, KeyEventKind, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc;

/// Application-level events funneled from crossterm and a periodic tick timer.
#[derive(Debug)]
pub enum Event {
    /// A keyboard key was pressed.
    Key(KeyEvent),
    /// A mouse action occurred.
    Mouse(MouseEvent),
    /// The terminal was resized to (columns, rows).
    Resize(u16, u16),
    /// A periodic tick — drives UI refresh and background work.
    Tick,
}

/// Bridges crossterm's async `EventStream` with the application event loop.
///
/// Spawns a background tokio task that multiplexes terminal events with a
/// fixed-interval tick.  Consumers call [`EventHandler::next`] to receive
/// the next event.
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    // Held to keep the spawned task alive; dropped when `EventHandler` is
    // dropped, which cancels the task via the closed channel.
    _task: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    /// Create a new event handler with the given tick rate (in milliseconds).
    pub fn new(tick_rate_ms: u64) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval = tokio::time::interval(Duration::from_millis(tick_rate_ms));

            loop {
                let tick_delay = tick_interval.tick();
                let crossterm_event = reader.next();

                tokio::select! {
                    _ = tick_delay => {
                        if tx.send(Event::Tick).is_err() {
                            // Receiver dropped; shut down.
                            break;
                        }
                    }
                    maybe_event = crossterm_event => {
                        match maybe_event {
                            Some(Ok(event)) => {
                                let mapped = match event {
                                    CrosstermEvent::Key(key) => {
                                        // Only forward actual key-press events
                                        // (ignore Release / Repeat on supported
                                        // terminals).
                                        if key.kind == KeyEventKind::Press {
                                            Some(Event::Key(key))
                                        } else {
                                            None
                                        }
                                    }
                                    CrosstermEvent::Mouse(mouse) => Some(Event::Mouse(mouse)),
                                    CrosstermEvent::Resize(w, h) => Some(Event::Resize(w, h)),
                                    // FocusGained, FocusLost, Paste — ignored.
                                    _ => None,
                                };

                                if let Some(app_event) = mapped {
                                    if tx.send(app_event).is_err() {
                                        break;
                                    }
                                }
                            }
                            Some(Err(_)) => {
                                // Stream error; nothing useful we can do.
                                break;
                            }
                            None => {
                                // Stream ended.
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self { rx, _task: task }
    }

    /// Wait for and return the next event.
    ///
    /// Returns an error if the internal channel has been closed (i.e. the
    /// background task exited).
    pub async fn next(&mut self) -> anyhow::Result<Event> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }
}
