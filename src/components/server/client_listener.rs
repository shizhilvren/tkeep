use super::super::Component;
use super::pty;
#[allow(unused_imports)]
use crate::action::Action::Server as s_action_e;
#[allow(unused_imports)]
use crate::action::server::Action as s_action;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::{action, event};
use bincode::{Decode, Encode};
use color_eyre::{Result, eyre::eyre};
use std::option::Option;
use std::path::PathBuf;
use strum::Display;
use tokio::net::{UnixListener, UnixStream};
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
#[allow(unused_imports)]
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
    name: String,
}

impl ClinetListener {
    pub fn new(name: String) -> Self {
        Self {
            event_tx: None,
            action_rx: None,
            name,
        }
    }

    async fn handle_action(
        action: Option<action::Action>,
        _event_tx: &UnboundedSender<event::Event>,
    ) -> Result<bool> {
        match action {
            Some(action) => match action {
                s_action_e(s_action::Pty(pty::Action::PtyFinish)) => Ok(true),
                _ => Ok(false),
            },
            None => {
                error!("Failed to receive action");
                return Err(eyre!("Failed to receive action"));
            }
        }
    }

    async fn start_listen(
        event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
        name: String,
    ) -> Result<()> {
        let event_tx = event_tx;
        let path = PathBuf::from(format!("/tmp/{}.tkeep.sock", name));
        debug!("Starting client listener on {:?}", path);
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => {
                debug!("Listening on {:?}", path);
                Ok(listener)
            }
            Err(e) => {
                error!("Failed to bind to {:?}: {}", path, e);
                Err(e)
            }
        }?;
        loop {
            select! {
                action = action_rx.recv() => {
                    if Self::handle_action(action, &event_tx).await?{
                        break;
                    }
                },
                pair = listener.accept() => {
                    let (stream, addr) = pair?;
                    event_tx.send(s_event_e(s_event::ClinetListener(Event::NewClient(stream))))?;
                    debug!("New client connected: {:?}", addr);
                }
            };
        }
        debug!("client listen finish");
        Ok(())
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
        debug!("Starting client listener with name: {}", self.name);
        match (event_rx, action_tx) {
            (Some(event_rx), Some(action_tx)) => {
                let task = Self::start_listen(event_rx, action_tx, self.name.clone());
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => {
                error!("Failed to get event or action channel");
                Err(eyre!("Failed to get event or action channel"))
            }
        }
    }
    fn action_filter(&mut self, action: &action::Action) -> bool {
        match action {
            s_action_e(s_action::Pty(pty::Action::PtyFinish)) => true,
            _ => false,
        }
    }
}
