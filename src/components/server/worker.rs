use super::super::Component;
use super::pty::{self, Pty};
use super::pty_buffer;
use crate::action::Action::Server as s_action_e;
use crate::action::server::Action as s_action;
use crate::components::PID;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::message::Msg;
use crate::{action, event, tool};
use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};
use std::{option::Option, str::FromStr};
use strum::Display;
use tokio::net::{UnixListener, UnixStream};
use tokio::select;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::codec::FramedRead;
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    PtyOut((PID, Vec<u8>)),
}
#[derive(Debug, Display)]

pub enum Event {
    Replay(PID),
}

#[derive(Debug, Default)]
pub struct Worker {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    client_uds: Option<UnixStream>,
    pid: Option<PID>,
    replay_finish: bool,
}

impl Worker {
    pub fn new(uds: UnixStream) -> Self {
        Worker {
            event_tx: None,
            action_rx: None,
            client_uds: Some(uds),
            pid: None,
            replay_finish: false,
        }
    }
    async fn start_client_worker(
        event_tx: UnboundedSender<event::Event>,
        action_rx: UnboundedReceiver<action::Action>,
        mut uds: UnixStream,
        pid: PID,
    ) -> Result<()> {
        let mut action_rx = action_rx;
        let event_tx = event_tx;
        event_tx.send(s_event_e(s_event::Worker(Event::Replay(pid.clone()))))?;
        let handle_action = async |action: Option<action::Action>,
                                   uds: &mut UnixStream|
               -> Result<()> {
            let action = action.ok_or(eyre!("Action is None"))?;
            match action {
                s_action_e(s_action::Worker(Action::PtyOut((pid_s, data)))) => {
                    if pid_s == pid {
                        if let Err(e) =
                            tool::unix_socket::send_message(uds, &Msg::PtyOut(data)).await
                        {
                            error!("Failed to send message: {}", e);
                            return Err(eyre!("Failed to send message"));
                        }
                    }
                }
                s_action_e(s_action::PtyBuffer(pty_buffer::Action::ReplayData((pid_s, data)))) => {
                    if pid_s == pid {
                        if let Err(e) =
                            tool::unix_socket::send_message(uds, &Msg::Replay(data)).await
                        {
                            error!("Failed to send message: {}", e);
                            return Err(eyre!("Failed to send message"));
                        }
                    }
                }
                _ => {}
            };
            Ok(())
        };
        let a = FramedRead::new(uds, tool::unix_socket::MessageReader::<Msg>{});
        loop {
            select! {
                action = action_rx.recv() => {
                    handle_action(action,&mut uds).await?;
                },
                data = tool::unix_socket::receive_message::<Msg>(&mut uds) => {
                    match data? {
                        Msg::PtyIn(data) => {
                            event_tx.send(s_event_e(s_event::Pty(pty::Event::PtyIn(data))))?;
                        }
                        Msg::Resize { width, height } => {
                            event_tx.send(s_event_e(s_event::Pty(pty::Event::Resize { width, height })))?;
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
    fn register_id(&mut self, pid: PID) -> Result<()> {
        self.pid = Some(pid);
        Ok(())
    }
    fn run(&mut self) -> Result<Option<tokio::task::JoinHandle<Result<()>>>> {
        let event_rx = self.event_tx.clone();
        let action_tx = self.action_rx.take();
        let uds = self.client_uds.take();
        match (event_rx, action_tx, uds, self.pid) {
            (Some(event_rx), Some(action_tx), Some(uds), Some(pid)) => {
                let task = Self::start_client_worker(event_rx, action_tx, uds, pid.clone());
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
    fn handle_events(&mut self, event: &event::Event) -> Result<Vec<action::Action>> {
        let mut ret = vec![];
        let pid = self.pid.ok_or(eyre!("PID is not set"))?;
        match event {
            s_event_e(s_event::Worker(Event::Replay(pid))) => {
                if Some(pid.clone()) == self.pid {
                    self.replay_finish = true;
                }
            }
            s_event_e(s_event::PtyBuffer(pty_buffer::Event::BufferToken(token))) => {
                if self.replay_finish {
                    ret.push(s_action_e(s_action::Worker(Action::PtyOut((
                        pid,
                        token.buf.clone(),
                    )))));
                }
            }
            _ => {}
        };
        Ok(ret)
    }
    fn action_filter(&mut self, action: &action::Action) -> bool {
        match action {
            s_action_e(s_action::Worker(Action::PtyOut((pid, _)))) => {
                if let Some(worker_pid) = self.pid.as_ref() {
                    return pid == worker_pid;
                }
            }
            s_action_e(s_action::PtyBuffer(pty_buffer::Action::ReplayData((pid, _)))) => {
                if let Some(worker_pid) = self.pid.as_ref() {
                    return pid == worker_pid;
                }
            }
            _ => {}
        }
        false
    }
}
