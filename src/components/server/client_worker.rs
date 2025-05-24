use super::super::Component;
use super::pty::{self, Pty};
use crate::message::Msg;
use crate::{action, event, tool};
use bincode::{Decode, Encode};
use color_eyre::{Result, eyre::eyre};
use std::{option::Option, str::FromStr};
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Encode, Decode)]
pub enum Action {}
#[derive(Debug, Display)]

pub enum Event {}

#[derive(Debug, Default)]
pub struct ClinetWorker {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    client_uds: Option<UnixStream>,
}

impl ClinetWorker {
    pub fn new(uds: UnixStream) -> Self {
        ClinetWorker {
            event_tx: None,
            action_rx: None,
            client_uds: Some(uds),
        }
    }
    async fn start_client_worker(
        event_tx: UnboundedSender<event::Event>,
        action_rx: UnboundedReceiver<action::Action>,
        mut uds: UnixStream,
    ) -> Result<()> {
        let mut action_rx = action_rx;
        let event_tx = event_tx;
        loop {
            select! {
                action = action_rx.recv() => {
                    match action {
                        Some(action) => {
                            // Handle the action here
                            debug!("Received action: {:?}", action);
                        }
                        None => {
                            error!("Failed to receive action");
                            return Err(eyre!("Failed to receive action"));
                        }
                    }
                },
                data = tool::unix_socket::receive_message::<Msg>(&mut uds) => {
                    match data? {
                        Msg::PtyIn(data) => {
                            event_tx.send(event::Event::Pty(pty::Event::PtyIn(data)))?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

impl Component for ClinetWorker {
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
        let uds = self.client_uds.take();
        match (event_rx, action_tx, uds) {
            (Some(event_rx), Some(action_tx), Some(uds)) => {
                let task = Self::start_client_worker(event_rx, action_tx, uds);
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
}
