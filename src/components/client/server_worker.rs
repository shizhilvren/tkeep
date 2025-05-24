use super::super::Component;
use crate::{action, event};
use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::net::UnixStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    StartClientPlayer,
}

#[derive(Debug, Default)]
pub struct ClientCommunication {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
}

impl ClientCommunication {
    pub fn new() -> Self {
        ClientCommunication::default()
    }
    async fn start_player(
        mut event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let mut stream = UnixStream::connect("/tmp/stream.sock").await?;
        
        Ok(())
    }
}

impl Component for ClientCommunication {
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
        Ok(ret)
    }
}
