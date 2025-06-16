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
use tracing::warn;
#[allow(unused_imports)]
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    PtyIn(Vec<u8>),
    Resize { width: u16, height: u16 },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    Start(String),
    Stop { reason: String },
}

#[derive(Debug, Default)]
pub struct Worker {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    name: String,
}

impl Worker {
    pub fn new(name: String) -> Self {
        Self {
            event_tx: None,
            action_rx: None,
            name,
        }
    }
    async fn handle_action(action: Option<action::Action>, uds: &mut UnixStream) -> Result<bool> {
        let action = action.ok_or(eyre!("Action is None"))?;
        let ret = match action {
            c_action_e(c_action::Worker(self::Action::PtyIn(data))) => {
                let msg = Msg::PtyIn(data);
                tool::unix_socket::send_message(uds, &msg).await?;
                false
            }
            c_action_e(c_action::Worker(self::Action::Resize { width, height })) => {
                let msg = Msg::Resize { width, height };
                tool::unix_socket::send_message(uds, &msg).await?;
                false
            }
            c_action_e(c_action::Worker(self::Action::Stop)) => true,
            _ => false,
        };
        Ok(ret)
    }
    async fn handle_msg(msg: Result<Msg>, event_tx: &UnboundedSender<event::Event>) -> Result<()> {
        match msg {
            Ok(data) => match data {
                Msg::PtyOut(data) => {
                    event_tx.send(c_event_e(c_event::Output(output::Event::PtyOut(data))))?;
                }
                Msg::Replay(data) => {
                    event_tx.send(c_event_e(c_event::Output(output::Event::Replay(data))))?;
                }
                _ => {}
            },
            Err(e) => {
                warn!("client worker stopped, reason {:?}", e);
                event_tx.send(c_event_e(c_event::Worker(Event::Stop {
                    reason: format!("client stop because server stop"),
                })))?;
            }
        }
        Ok(())
    }
    async fn start_player(
        event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
        name: String,
    ) -> Result<()> {
        let uds = UnixStream::connect(tool::CFG.sock(&name)).await;
        let mut uds = match uds {
            Err(e) => {
                event_tx.send(c_event_e(c_event::Worker(Event::Stop {
                    reason: format!("cannot client server named '{}'", &name),
                })))?;
                error!("clinet fail: {:}", e);
                Err(eyre!("cannot client server named '{}'.", &name))
            }
            Ok(uds) => Ok(uds),
        }?;
        event_tx.send(c_event_e(c_event::Input(input::Event::Start)))?;
        let mut msg_receive = tool::unix_socket::MessageReader::new();
        loop {
            select! {
                action = action_rx.recv() => {
                    if Self::handle_action(action, &mut uds).await? {
                        break;
                    }
                },
                data = msg_receive.receive_message::<Msg>(&mut uds) => {
                    Self::handle_msg(data, &event_tx).await?;
                }
            }
        }
        debug!("client worker finish");
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
                let task = Self::start_player(event_rx, action_tx, self.name.clone());
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
            c_event_e(c_event::Input(input::Event::Resize { width, height })) => {
                ret.push(c_action_e(c_action::Worker(Action::Resize {
                    width: *width,
                    height: *height,
                })));
            }
            c_event_e(c_event::Worker(Event::Stop { .. })) => {
                ret.push(c_action_e(c_action::Worker(Action::Stop)));
            }
            _ => {}
        }
        Ok(ret)
    }
    fn action_filter(&mut self, action: &action::Action) -> bool {
        match action {
            c_action_e(c_action::Worker(_)) => true,
            _ => false,
        }
    }
}
