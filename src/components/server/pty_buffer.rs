use super::super::Component;
use super::pty;
use crate::action::Action::Server as s_action_e;
use crate::action::server::Action as s_action;
use crate::components::PID;
use crate::components::server::worker;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::tool;
use crate::{action, event};
use bytes::buf;
use color_eyre::{Result, eyre::eyre};
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::mem;
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error};
use tracing_subscriber::field::debug;
use vte::{Params, Parser, Perform};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    BufferIn(Vec<u8>),
    ReplayData((PID, Vec<u8>)),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    BufferToken(PtyOutputToken),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PtyOutputToken {
    action: String,
    mean: VTEEvent,
    pub buf: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize, Default)]
enum VTEEvent {
    #[default]
    Print,
    Execute(u8),
    Hook,
    Put,
    UnHook,
    OscDispatch {
        params: Vec<Vec<u8>>,
        bell_terminated: bool,
    },
    CsiDispatch {
        params: Vec<Vec<u16>>,
        intermediates: Vec<u8>,
        ignore: bool,
        c: char,
    },
    EscDispatch,
}

#[derive(Debug, Default)]
pub struct PtyBuffer {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    output_buf: VecDeque<PtyOutputToken>,
}
#[derive(Default)]
struct TokenBuffer {
    buffer: Vec<u8>,
    statemachine: Option<Parser>,
    token: Option<PtyOutputToken>,
}

impl Perform for TokenBuffer {
    fn print(&mut self, c: char) {
        let msg = format!("[print] {:?}", c);
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::Print,
            action: msg,
        });
    }

    fn execute(&mut self, byte: u8) {
        let msg = format!("[execute] {:02x}", byte);
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::Print,
            action: msg,
        });
    }

    fn hook(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
        let msg = format!(
            "[hook] params={:?}, intermediates={:?}, ignore={:?}, char={:?}",
            params, intermediates, ignore, c
        );
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::Print,
            action: msg,
        });
    }

    fn put(&mut self, byte: u8) {
        let msg = format!("[put] {:02x}", byte);
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::Print,
            action: msg,
        });
    }

    fn unhook(&mut self) {
        let msg = format!("[unhook]");
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::Print,
            action: msg,
        });
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        let msg = format!(
            "[osc_dispatch] params={:?} bell_terminated={}",
            params, bell_terminated
        );
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::OscDispatch {
                params: params.iter().map(|&p| p.to_vec()).collect(),
                bell_terminated,
            },
            action: msg,
        });
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
        let msg = format!(
            "[csi_dispatch] params={:#?} , intermediates={:?}, ignore={:?}, char={:?}",
            params, intermediates, ignore, c
        );
        let a = params.iter().map(|e| e).collect::<Vec<_>>();
        debug!(
            "[csi_dispatch] params={:?} intermediates={:?}, ignore={:?}, char={:?}",
            a, intermediates, ignore, c
        );
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::CsiDispatch {
                params: params.iter().map(|e| e.to_vec()).collect(),
                intermediates: intermediates.to_vec(),
                ignore,
                c,
            },
            action: msg,
        });
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        let msg = format!(
            "[esc_dispatch] intermediates={:?}, ignore={:?}, byte={:02x}",
            intermediates, ignore, byte
        );
        debug!("{}", &msg);
        let mut buf = vec![];
        mem::swap(&mut self.buffer, &mut buf);
        self.token = Some(PtyOutputToken {
            buf,
            mean: VTEEvent::Print,
            action: msg,
        });
    }
    fn terminated(&self) -> bool {
        debug!("[terminated]");
        false
    }
}

impl TokenBuffer {
    pub fn new() -> Self {
        TokenBuffer::default()
    }
    pub fn advance(&mut self, data: &[u8]) -> Vec<PtyOutputToken> {
        let mut ret = vec![];
        let mut statemachine = match self.statemachine.take() {
            Some(statemachine) => statemachine,
            None => Parser::new(),
        };
        data.iter().for_each(|&byte| {
            self.buffer.push(byte);
            statemachine.advance(self, &[byte]);
            if let Some(token) = self.token.take() {
                ret.push(token);
            }
        });
        self.statemachine = Some(statemachine);
        ret
    }
}

impl PtyBuffer {
    pub fn new() -> Self {
        PtyBuffer::default()
    }
    async fn start_pty_buffer(
        mut event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let mut token_buffer = TokenBuffer::new();
        loop {
            let action = action_rx.recv().await.ok_or(eyre!("action not get"))?;
            match action {
                s_action_e(s_action::PtyBuffer(Action::BufferIn(data))) => {
                    let tokens = token_buffer.advance(&data);
                    for token in tokens {
                        event_tx.send(s_event_e(s_event::PtyBuffer(Event::BufferToken(token))))?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl Component for PtyBuffer {
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
                let task = Self::start_pty_buffer(event_rx, action_tx);
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
    fn handle_events(&mut self, event: &event::Event) -> Result<Vec<action::Action>> {
        let mut ret = vec![];
        match event {
            s_event_e(s_event::Pty(pty::Event::PtyOut(data))) => {
                ret.push(s_action_e(s_action::PtyBuffer(Action::BufferIn(
                    data.clone(),
                ))));
            }
            s_event_e(s_event::PtyBuffer(Event::BufferToken(token))) => {
                self.output_buf.push_back(token.clone());
            }
            s_event_e(s_event::Worker(worker::Event::Replay(pid))) => {
                ret.push(s_action_e(s_action::PtyBuffer(Action::ReplayData((
                    pid.clone(),
                    self.output_buf
                        .iter()
                        .filter(|token| !token.mean.is_query())
                        .flat_map(|token| token.buf.clone())
                        .collect(),
                )))));
            }
            _ => {}
        };
        Ok(ret)
    }
}

impl VTEEvent {
    pub fn is_query(&self) -> bool {
        match self {
            VTEEvent::OscDispatch {
                params,
                bell_terminated,
            } => false,
            VTEEvent::CsiDispatch {
                params,
                intermediates,
                ignore,
                c,
            } => {
                let params = params.iter().map(|e| e.as_slice()).collect::<Vec<_>>();
                let params = params.as_slice();
                let intermediates = intermediates.as_slice();
                match (params, intermediates, ignore, c) {
                    ([[6_u16]], [], false, 'n') => true,   //光标位置查询
                    ([[0_u16]], [62], false, 'c') => true, //设备属性查询
                    // (_, _, _, 'c') => true,
                    // (_, _, _, 'C') => true,
                    // (_, _, _, 'h') => true,
                    // (_, _, _, 'H') => true,
                    // (_, _, _, 'l') => true,
                    // (_, _, _, 'm') => true,
                    // (_, _, _, 'n') => true,
                    (_, _, _, 'J') => true,
                    (_, _, _, 'K') => true,
                    // (_, _, _, 'p') => true,
                    // (_, _, _, 'r') => true,
                    (_, _, _, 't') => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}
