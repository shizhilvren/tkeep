use super::super::Component;
use super::{output, worker};
use crate::action::Action::Clinet as c_action_e;
use crate::action::client::Action as c_action;
use crate::event::Event::Client as c_event_e;
use crate::event::client::Event as c_event;
use crate::{action, event, tool};
use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;
#[allow(unused_imports)]
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    Start,
    PtyIn(Vec<u8>),
    Resize { width: u16, height: u16 },
}

#[derive(Debug, Default)]
pub struct Input {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
}

impl Input {
    pub fn new() -> Self {
        Input::default()
    }
    async fn start_input_loop(
        event_tx: UnboundedSender<event::Event>,
        _action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        match crossterm::terminal::enable_raw_mode() {
            Ok(()) => {
                let mut resize_signals =
                    signal_hook_tokio::Signals::new([signal_hook::consts::SIGWINCH])?;
                let mut tty = tokio::fs::File::open("/dev/tty").await?;
                let mut buf = [0_u8; tool::BUF_SIZE];
                loop {
                    select! {
                         n = tty.read(&mut buf) => {
                            let n = n?;
                            event_tx.send(c_event_e(c_event::Input(Event::PtyIn(buf[..n].to_vec()))))?;
                         }

                         resize = resize_signals.next() => {
                            match resize {
                                Some(signal_hook::consts::SIGWINCH) => {
                                    let size = crossterm::terminal::size()?;
                                    event_tx.send(c_event_e(c_event::Input(Event::Resize { width: size.0, height: size.1 })))?;
                                }
                                _ => {}
                            }
                         }
                    }
                }
            }
            Err(e) => {
                error!("tty not change to raw model {:?}", e);
            }
        }
        Ok(())
    }
}

impl Component for Input {
    fn register_action_handler(&mut self) -> Result<Option<UnboundedSender<action::Action>>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.action_rx = Some(rx);
        Ok(Some(tx))
    }
    fn register_event_handler(&mut self, tx: UnboundedSender<event::Event>) -> Result<()> {
        self.event_tx = Some(tx);
        Ok(())
    }
    fn run(&mut self) -> Result<Option<tokio::task::JoinHandle<Result<()>>>> {
        let event_rx = self.event_tx.clone();
        let action_tx = self.action_rx.take();
        match (event_rx, action_tx) {
            (Some(event_rx), Some(action_tx)) => {
                let task = Self::start_input_loop(event_rx, action_tx);
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
    fn handle_events(&mut self, event: &event::Event) -> Result<Vec<action::Action>> {
        let mut ret = vec![];
        match event {
            c_event_e(c_event::Output(output::Event::Replay(_))) => {
                let size = crossterm::terminal::size()?;
                ret.push(c_action_e(c_action::Worker(worker::Action::Resize {
                    width: size.0,
                    height: size.1,
                })));
            }
            _ => {}
        };
        Ok(ret)
    }
}
