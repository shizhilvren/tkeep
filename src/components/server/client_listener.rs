use super::super::Component;
use crate::action::Action::Server as s_action_e;
use crate::action::server::Action as s_action;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::{action, event};
use bincode::{Decode, Encode};
use bytes::buf;
use color_eyre::{Result, eyre::eyre};
use std::str::from_utf8;
use std::{option::Option, str::FromStr};
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Encode, Decode)]
pub enum Action {}
#[derive(Debug, Display)]
pub enum Event {
    NewClient(UnixStream),
}

#[derive(Debug, Default)]
pub struct ClinetListener {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
}

impl ClinetListener {
    pub fn new() -> Self {
        ClinetListener::default()
    }
    async fn start_listen(
        event_tx: UnboundedSender<event::Event>,
        action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let mut action_rx = action_rx;
        let event_tx = event_tx;
        let path = "/tmp/stream.sock";
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        loop {
            let (mut stream, addr) = listener.accept().await?;
            event_tx.send(s_event_e(s_event::ClinetListener(Event::NewClient(stream))))?;
            debug!("New client connected: {:?}", addr);
        }
    }
}

impl Component for ClinetListener {
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
                let task = Self::start_listen(event_rx, action_tx);
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
}
