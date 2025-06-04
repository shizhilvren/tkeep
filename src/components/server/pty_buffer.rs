use super::super::Component;
use super::pty;
use crate::action::Action::Server as s_action_e;
use crate::action::server::Action as s_action;
use crate::components::PID;
use crate::components::server::worker;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::{action, event, tool};
use bytes::{BufMut, Bytes, BytesMut};
use color_eyre::{Result, eyre::eyre};
use strum::Display;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
#[allow(unused_imports)]
use tracing::{debug, error, trace};

#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Action {
    BufferIn(Vec<u8>),
    ReplayData((PID, Vec<u8>)),
}

#[derive(Debug, Clone, Display)]
pub enum Event {
    BufferTokens(Vec<paser::Token>),
}

#[derive(Debug, Default)]
pub struct PtyBuffer {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    output_buf: BytesMut,
}

mod paser {
    use std::collections::VecDeque;

    use bytes::{BufMut, Bytes, BytesMut};

    #[derive(Default)]
    pub struct Paser {
        buffer: BytesMut,
        paser: termwiz::escape::parser::Parser,
        vt100: vt100::Parser,
    }

    #[derive(Debug, Clone)]
    pub struct Token {
        pub buf: Bytes,
        pub mean: Mean,
        in_alternate_screen: bool,
        cut_point: Option<CutPoint>,
    }

    #[derive(Debug, Clone)]
    pub struct CutPoint(vt100::Screen);
    #[derive(Debug, Clone)]
    pub struct Mean(termwiz::escape::Action);

    #[derive(Debug, Clone)]
    struct PtyReplayBuffer {
        now_point: Option<CutPoint>,
        cut_parts: VecDeque<PtyReplayBufferOne>,
        part: Option<PtyReplayBufferOne>,
    }

    #[derive(Debug, Clone)]
    struct PtyReplayBufferOne {
        buffer: BytesMut,
        start: CutPoint,
    }

    impl Paser {
        pub fn new(rows: u16, cols: u16) -> Self {
            Self {
                buffer: BytesMut::new(),
                paser: termwiz::escape::parser::Parser::new(),
                vt100: vt100::Parser::new(rows, cols, 0),
            }
        }
        pub fn advance(&mut self, data: Bytes) -> Vec<Token> {
            let mut actions: Vec<(Mean, Bytes)> = vec![];
            self.buffer.put(data);
            while let Some((action, len)) = self.paser.parse_first(&self.buffer) {
                let token = self.buffer.split_to(len);
                actions.push((Mean(action), token.into()));
            }
            actions
                .into_iter()
                .map(|(mean, buf)| {
                    self.vt100.process(&buf);
                    let is_cut = mean.is_newline();
                    let screen = self.vt100.screen();
                    let alternate_screen = screen.alternate_screen();
                    let cut_point = match (is_cut, alternate_screen) {
                        (true, false) => Some(CutPoint(screen.clone())),
                        _ => None,
                    };
                    Token {
                        buf,
                        mean,
                        in_alternate_screen: alternate_screen,
                        cut_point,
                    }
                })
                .collect()
        }
    }

    // reference: link
    // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
    impl Mean {
        pub fn is_newline(&self) -> bool {
            use termwiz::escape::Action;
            use termwiz::escape::ControlCode;
            match &self.0 {
                Action::Control(ctl) => match *ctl {
                    ControlCode::LineFeed => true,
                    _ => false,
                },
                _ => false,
            }
        }
        pub fn is_query(&self) -> bool {
            use termwiz::escape::Action;
            use termwiz::escape::csi;
            use termwiz::escape::osc;
            match &self.0 {
                Action::OperatingSystemCommand(cmd) => match **cmd {
                    osc::OperatingSystemCommand::QuerySelection(..) => true,
                    _ => false,
                },
                Action::CSI(csi) => match csi {
                    csi::CSI::Cursor(cursor) => match cursor {
                        csi::Cursor::RequestActivePositionReport => true,
                        _ => false,
                    },
                    csi::CSI::Mode(mode) => match mode {
                        csi::Mode::QueryDecPrivateMode(..) => true,
                        csi::Mode::QueryMode(..) => true,
                        _ => false,
                    },
                    csi::CSI::Device(device) => match **device {
                        csi::Device::RequestPrimaryDeviceAttributes => true,
                        csi::Device::RequestTerminalNameAndVersion => true,
                        csi::Device::RequestTerminalParameters(..) => true,
                        csi::Device::RequestSecondaryDeviceAttributes => true,
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            }
        }
    }
}

impl PtyBuffer {
    pub fn new() -> Self {
        PtyBuffer::default()
    }
    async fn start_pty_buffer(
        event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let mut token_buffer = paser::Paser::new(tool::TTY_SIZE.0, tool::TTY_SIZE.1);
        loop {
            let action = action_rx.recv().await.ok_or(eyre!("action not get"))?;
            match action {
                s_action_e(s_action::PtyBuffer(Action::BufferIn(data))) => {
                    let tokens = token_buffer
                        .advance(Bytes::from_owner(data))
                        .into_iter()
                        .filter(|token| !token.mean.is_query())
                        .collect();
                    event_tx.send(s_event_e(s_event::PtyBuffer(Event::BufferTokens(tokens))))?;
                }
                s_action_e(s_action::Pty(pty::Action::PtyFinish)) => {
                    break;
                }
                _ => {}
            }
        }
        debug!("pty buffer finish");
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
            s_event_e(s_event::PtyBuffer(Event::BufferTokens(tokens))) => {
                trace!(
                    "buffer size {} MB",
                    self.output_buf.len() as f64 * size_of::<u8>() as f64 / 4_f64 / 1024_f64
                );
                let buf = tokens
                    .into_iter()
                    .map(|token| token.buf.clone())
                    .flatten()
                    .collect::<Vec<_>>();
                self.output_buf.put(buf.as_slice());
            }
            s_event_e(s_event::Worker(worker::Event::Replay(pid))) => {
                ret.push(s_action_e(s_action::PtyBuffer(Action::ReplayData((
                    pid.clone(),
                    self.output_buf
                        .iter()
                        .map(|e| e.clone())
                        // .filter(|token| !token.mean.is_query())
                        .collect::<Vec<u8>>(),
                )))));
            }
            _ => {}
        };
        Ok(ret)
    }
    fn action_filter(&mut self, action: &action::Action) -> bool {
        match action {
            s_action_e(s_action::PtyBuffer(Action::BufferIn(_))) => true,
            s_action_e(s_action::Pty(pty::Action::PtyFinish)) => true,
            _ => false,
        }
    }
}
