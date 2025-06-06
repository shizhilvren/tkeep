use super::super::Component;
use super::pty;
use crate::action::Action::Server as s_action_e;
use crate::action::server::Action as s_action;
use crate::components::PID;
use crate::components::server::worker;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::{action, event};
use bytes::{BufMut, Bytes};
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
    BufferTokens((Vec<paser::Token>, paser::CutPoint)),
}

#[derive(Debug)]
pub struct PtyBuffer {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    output_buf: paser::PtyReplayBuffer,
    paser: Option<paser::Paser>,
}

pub mod paser {
    use bytes::{BufMut, Bytes, BytesMut};
    use crossterm::Command;
    use std::{collections::VecDeque, fmt::Debug};
    use termwiz::escape::CSI;
    use tracing::{debug, error};

    #[derive(Default)]
    pub struct Paser {
        buffer: BytesMut,
        paser: termwiz::escape::parser::Parser,
        vt100: vt100::Parser,
    }

    #[derive(Clone)]
    pub struct Token {
        pub buf: Bytes,
        pub mean: Mean,
        in_alternate_screen: bool,
        cut_point: Option<CutPoint>, // this pot is before do buffer
    }

    #[derive(Debug, Clone)]
    pub struct CutPoint(vt100::Screen);
    #[derive(Debug, Clone)]
    pub struct Mean(termwiz::escape::Action);

    #[derive(Debug, Clone)]
    pub struct PtyReplayBuffer {
        now_point: CutPoint,
        cut_parts: VecDeque<PtyReplayBufferOne>,
        part: PtyReplayBufferOne,
        history: u32,
    }

    #[derive(Debug, Clone)]
    pub struct PtyReplayBufferOne {
        buffer: BytesMut,
        start: Bytes,
    }

    impl Paser {
        pub fn new(rows: u16, cols: u16) -> Self {
            Self {
                buffer: BytesMut::new(),
                paser: termwiz::escape::parser::Parser::new(),
                vt100: vt100::Parser::new(rows, cols, 0),
            }
        }
        pub fn resize(&mut self, rows: u16, cols: u16) {
            self.vt100.set_size(rows, cols);
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
                    let screen = self.vt100.screen();
                    let alternate_screen: bool = screen.alternate_screen()
                        || (mean.enter_alternate_screen() || mean.leave_alternate_screen());

                    let alternate_screen: bool =
                        if screen.alternate_screen() && mean.enter_alternate_screen() {
                            false
                        } else {
                            screen.alternate_screen()
                        };
                    let is_cut = mean.is_newline();
                    let cut_point = match (is_cut, alternate_screen) {
                        (true, false) => Some(CutPoint(screen.clone())),
                        _ => None,
                    };
                    // let cut_point = None;
                    Token {
                        buf,
                        mean,
                        in_alternate_screen: alternate_screen,
                        cut_point,
                    }
                })
                .collect()
        }
        pub fn get_screen(&self) -> CutPoint {
            CutPoint(self.vt100.screen().clone())
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
        pub fn enter_alternate_screen(&self) -> bool {
            use termwiz::escape::Action;
            use termwiz::escape::csi;
            match &self.0 {
                Action::CSI(CSI::Mode(csi::Mode::SetDecPrivateMode(
                    csi::DecPrivateMode::Code(code),
                ))) => match code {
                    csi::DecPrivateModeCode::ClearAndEnableAlternateScreen => true,
                    csi::DecPrivateModeCode::OptEnableAlternateScreen => true,
                    _ => false,
                },
                _ => false,
            }
        }
        pub fn leave_alternate_screen(&self) -> bool {
            use termwiz::escape::Action;
            use termwiz::escape::csi;
            match &self.0 {
                Action::CSI(CSI::Mode(csi::Mode::ResetDecPrivateMode(
                    csi::DecPrivateMode::Code(code),
                ))) => match code {
                    csi::DecPrivateModeCode::ClearAndEnableAlternateScreen => true,
                    csi::DecPrivateModeCode::OptEnableAlternateScreen => true,
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
    impl PtyReplayBufferOne {
        pub fn new(cp: &CutPoint) -> Self {
            Self {
                buffer: BytesMut::new(),
                start: Bytes::from_iter(
                    vec![
                        cp.0.attributes_formatted(),
                        cp.0.input_mode_formatted(),
                        cp.0.title_formatted(),
                        // cp.0.cursor_state_formatted(),
                    ]
                    .into_iter()
                    .flatten(),
                ),
            }
        }
        pub fn get_replay_buffer(&self) -> &BytesMut {
            &self.buffer
        }
        pub fn advance(&mut self, token: Token) -> bool {
            match token.in_alternate_screen {
                _ => {
                    let cut_end = token.mean.is_newline();
                    self.buffer.put(token.buf);
                    cut_end
                }
            }
        }
        pub fn len(&self) -> usize {
            self.buffer.len()
        }
        pub fn get_screen(&self) -> &Bytes {
            &self.start
        }
    }
    impl PtyReplayBuffer {
        pub fn new(cp: CutPoint, history: u32) -> Self {
            Self {
                part: PtyReplayBufferOne::new(&cp),
                now_point: cp,
                cut_parts: VecDeque::new(),
                history,
            }
        }
        pub fn last_mut(&mut self) -> &mut PtyReplayBufferOne {
            &mut self.part
        }
        pub fn first(&self) -> &PtyReplayBufferOne {
            match self.cut_parts.front() {
                Some(part) => part,
                None => &self.part,
            }
        }
        pub fn last_finish(&mut self, cp: &CutPoint) {
            let mut new_last = PtyReplayBufferOne::new(cp);
            std::mem::swap(&mut self.part, &mut new_last);
            self.cut_parts.push_back(new_last);
            while self.lines_number() as u64 > self.history as u64 {
                if let Some(part) = self.cut_parts.pop_front() {
                    debug!("remove part: {}", part.len());
                }
            }
        }
        pub fn get_replay_buffer(&self) -> Vec<u8> {
            use crossterm::{ExecutableCommand, cursor};
            let mut events = BytesMut::new();
            match crossterm::cursor::MoveTo(0, 0).write_ansi(&mut events) {
                Err(e) => {
                    error!(" move to 0 0 fail: {}", e);
                }
                Ok(_) => {}
            }

            let events = events.to_vec();

            let now = &self.now_point.0;
            let start = &self.first().get_screen();
            vec![start.to_vec()]
                .into_iter()
                .chain(
                    self.cut_parts
                        .iter()
                        .chain(std::iter::once(&self.part))
                        .map(|buf| buf.get_replay_buffer().to_vec()),
                )
                .chain(std::iter::once(match now.alternate_screen() {
                    true => vec![
                        events,
                        // now.input_mode_formatted(),
                        now.state_formatted(),
                        // now.attributes_formatted(),
                        // now.title_formatted(),
                        // now.contents_formatted(),
                        // now.cursor_state_formatted(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    false => vec![],
                }))
                // .chain(std::iter::once(match self.now_point.0.alternate_screen() {
                //     true => self.now_point.0.attributes_formatted(),
                //     false => vec![],
                // }))
                .flatten()
                .collect()
        }
        pub fn advance(&mut self, tokens: Vec<Token>) {
            tokens
                .into_iter()
                .filter(|token| !token.mean.is_query())
                .filter(|token| !token.in_alternate_screen)
                .for_each(|mut token| {
                    let screen = token.cut_point.take();
                    self.last_mut().advance(token);
                    screen.map(|s| {
                        self.last_finish(&s);
                    });
                });
        }
        pub fn update_screen(&mut self, cp: CutPoint) {
            self.now_point = cp;
        }
        pub fn len(&self) -> usize {
            self.cut_parts
                .iter()
                .chain(std::iter::once(&self.part))
                .map(|buf| buf.len())
                .sum()
        }
        pub fn lines_number(&self) -> usize {
            self.cut_parts.len().saturating_add(1)
        }
    }
    impl Debug for Paser {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Paser {{ {:?} }}", &self.buffer)
        }
    }
    impl Debug for Token {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "Token {{ buf: {:?} means: {:?} alternate_screen: {} }}",
                &self.buf, &self.mean, &self.in_alternate_screen
            )
        }
    }
}

impl PtyBuffer {
    pub fn new(paser: paser::Paser, history: u32) -> Self {
        Self {
            event_tx: None,
            action_rx: None,
            output_buf: paser::PtyReplayBuffer::new(paser.get_screen(), history),
            paser: Some(paser),
        }
    }
    async fn start_pty_buffer(
        event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
        mut token_buffer: paser::Paser,
    ) -> Result<()> {
        loop {
            let action = action_rx.recv().await.ok_or(eyre!("action not get"))?;
            match action {
                s_action_e(s_action::PtyBuffer(Action::BufferIn(data))) => {
                    let tokens = token_buffer
                        .advance(Bytes::from_owner(data))
                        .into_iter()
                        .collect();
                    let cp = token_buffer.get_screen();
                    event_tx.send(s_event_e(s_event::PtyBuffer(Event::BufferTokens((
                        tokens, cp,
                    )))))?;
                }
                s_action_e(s_action::Pty(pty::Action::Resize { width, height })) => {
                    token_buffer.resize(height, width);
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
        let token_buffer = self.paser.take().ok_or(eyre!("Paser not initialized"))?;
        // self.output_buf = Some(paser::PtyReplayBuffer::new(token_buffer.get_screen()));
        match (event_rx, action_tx) {
            (Some(event_rx), Some(action_tx)) => {
                let task = Self::start_pty_buffer(event_rx, action_tx, token_buffer);
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
            s_event_e(s_event::PtyBuffer(Event::BufferTokens((tokens, cp)))) => {
                trace!(
                    "buffer lines {} size {} MB",
                    self.output_buf.lines_number(),
                    self.output_buf.len() as f64 / 1024_f64 / 1024_f64
                );
                self.output_buf.advance(tokens.clone());
                self.output_buf.update_screen(cp.clone());
            }
            s_event_e(s_event::Worker(worker::Event::Replay(pid))) => {
                ret.push(s_action_e(s_action::PtyBuffer(Action::ReplayData((
                    pid.clone(),
                    self.output_buf.get_replay_buffer(),
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
            s_action_e(s_action::Pty(pty::Action::Resize { .. })) => true,
            _ => false,
        }
    }
}
