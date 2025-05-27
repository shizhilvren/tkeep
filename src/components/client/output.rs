use std::io::Write;

use super::super::Component;
use crate::action::Action::Clinet as c_action_e;
use crate::action::client::Action as c_action;
use crate::event::Event::Client as c_event_e;
use crate::event::client::Event as c_event;
use crate::{action, event, tool};
use color_eyre::{Result, eyre::eyre};
use crossterm::{Command, execute};
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::error;

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    PtyOut(Vec<u8>),
    Replay(Vec<u8>),
}

#[derive(Debug, Default)]
pub struct Output {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
    reply_finish: bool,
}

impl Output {
    pub fn new() -> Self {
        Output::default()
    }
}

impl Component for Output {
    fn register_action_handler(&mut self) -> Result<Option<UnboundedSender<action::Action>>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.action_rx = Some(rx);
        Ok(Some(tx))
    }
    fn register_event_handler(&mut self, tx: UnboundedSender<event::Event>) -> Result<()> {
        self.event_tx = Some(tx);
        Ok(())
    }

    fn handle_events(&mut self, event: &event::Event) -> Result<Vec<action::Action>> {
        let mut ret = vec![];
        let mut out = std::io::stdout();
        match event {
            c_event_e(c_event::Output(Event::PtyOut(data))) => {
                if self.reply_finish {
                    out.write_all(&data)
                        .map_err(|e| eyre!("Failed to write to stdout: {}", e))?;
                    out.flush()
                        .map_err(|e| eyre!("Failed to flush stdout: {}", e))?;
                }
            }
            c_event_e(c_event::Output(Event::Replay(data))) => {
                if !self.reply_finish {
                    self.reply_finish = true;
                    execute!(
                        out,
                        crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)
                    )?;
                    execute!(out, crossterm::terminal::LeaveAlternateScreen)?;
                    // execute!(out, crossterm::terminal::EnableLineWrap)?;
                    execute!(out, crossterm::style::ResetColor)?;
                    execute!(out, crossterm::cursor::MoveTo(0, 0))?;
                    execute!(
                        out,
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
                    )?;
                    // crossterm::terminal::enable_raw_mode()?;
                    out.write_all(&data)
                        .map_err(|e| eyre!("Failed to write to stdout: {}", e))?;
                    out.flush()
                        .map_err(|e| eyre!("Failed to flush stdout: {}", e))?;
                } else {
                    error!("Replay finished, ignoring replay data");
                }
            }
            _ => {}
        }
        Ok(ret)
    }
}
