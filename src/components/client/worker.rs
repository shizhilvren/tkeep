use super::super::Component;
use super::input;
use crate::action::Action::Clinet as c_action_e;
use crate::action::client::Action as c_action;
use crate::components::client::output;
use crate::event::Event::Client as c_event_e;
use crate::event::client::Event as c_event;
use crate::message::Msg;
use crate::{action, event, tool};
use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::net::UnixStream;
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    PtyIn(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    Start,
}

#[derive(Debug, Default)]
pub struct Worker {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
}

impl Worker {
    pub fn new() -> Self {
        Worker::default()
    }
    async fn start_player(
        mut event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let handle_action =
            async |action: Option<action::Action>, uds: &mut UnixStream| -> Result<()> {
                let action = action.ok_or(eyre!("Action is None"))?;
                match action {
                    c_action_e(c_action::Worker(self::Action::PtyIn(data))) => {
                        let msg = Msg::PtyIn(data);
                        tool::unix_socket::send_message(uds, &msg).await?;
                    }
                    _ => {}
                }
                Ok(())
            };
        let mut uds = UnixStream::connect("/tmp/stream.sock").await?;
        loop {
            select! {
                action = action_rx.recv() => {
                    handle_action(action,&mut uds).await?;
                },
                data = tool::unix_socket::receive_message::<Msg>(&mut uds) => {
                    match data? {
                        Msg::PtyOut(data) => {
                            event_tx.send(c_event_e(c_event::Output(output::Event::PtyOut(data))))?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

impl Component for Worker {
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
                let task = Self::start_player(event_rx, action_tx);
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
    fn handle_events(&mut self, event: &event::Event) -> Result<Vec<action::Action>> {
        let mut ret = vec![];
        match event {
            c_event_e(c_event::Input(input::Event::PtyIn(data))) => {
                ret.push(c_action_e(c_action::Worker(Action::PtyIn(data.clone()))));
            }
            _ => {}
        }
        Ok(ret)
    }
}
