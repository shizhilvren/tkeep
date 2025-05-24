use super::super::Component;
use crate::{action, event};
use color_eyre::{Result, eyre::eyre};
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::os::fd::FromRawFd;
use std::str::from_utf8;
use std::{option::Option, str::FromStr};
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    PtyIn(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Event {
    PtyOut(Vec<u8>),
    PtyIn(Vec<u8>),
}

#[derive(Debug, Default)]
pub struct Pty {
    event_tx: Option<UnboundedSender<event::Event>>,
    action_rx: Option<UnboundedReceiver<action::Action>>,
}

impl Pty {
    pub fn new() -> Self {
        Pty::default()
    }
    async fn handle_action(
        action: Option<action::Action>,
        event_tx: &UnboundedSender<event::Event>,
        tty_in: &mut Box<dyn std::io::Write + Send + 'static>,
    ) -> Result<()> {
        match action {
            Some(action) => match action {
                action::Action::Pty(self::Action::PtyIn(data)) => {
                    if let Err(e) = tty_in.write_all(&data) {
                        error!("Failed to write to pty: {}", e);
                        return Err(eyre!("Failed to write to pty"));
                    } else {
                        debug!("pty writee data: {:?}", from_utf8(&data));
                    }
                }
                _ => {}
            },
            None => {
                error!("Failed to receive action");
                return Err(eyre!("Failed to receive action"));
            }
        }
        Ok(())
    }
    async fn handle_output(
        event_tx: &mut UnboundedSender<event::Event>,
        output: Result<usize, std::io::Error>,
        buf: &[u8],
    ) -> Result<()> {
        match output {
            Ok(n) => {
                if n == 0 {
                    debug!("pty read 0 bytes");
                    return Err(eyre!("pty read 0 bytes"));
                }
                let data = buf[..n].to_vec();
                debug!("pty read data: {:?}", from_utf8(&data));
                event_tx.send(event::Event::Pty(Event::PtyOut(data)))?;
            }
            Err(e) => {
                error!("Failed to read from pty: {}", e);
                return Err(eyre!("Failed to read from pty"));
            }
        }
        Ok(())
    }
    async fn start_pty(
        mut event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                // Not all systems support pixel_width, pixel_height,
                // but it is good practice to set it to something
                // that matches the size of the selected font.  That
                // is more complex than can be shown here in this
                // brief example though!
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| eyre!(format!("{:?}", e)))?;
        let args = ["bash"]
            .iter()
            .copied()
            .map(|s| -> Result<std::ffi::OsString> { Ok(std::ffi::OsString::from_str(s)?) })
            .collect::<Result<Vec<_>>>()?;
        let cwd = std::env::current_dir()?;
        debug!("gdb tty start with {:?} currect dir is {:?}", &args, &cwd);
        // Spawn a shell into the pty
        let mut cmd = CommandBuilder::from_argv(args);
        cmd.cwd(cwd);
        debug!("gdb tty start cwd id {:?}", &cmd.get_cwd());
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| eyre!(format!("{:?}", e)))?;

        // // Read and parse output from the pty with reader
        // let tty_out = pair
        //     .master
        //     .try_clone_reader()
        //     .map_err(|e| eyre!(format!("{:?}", e)))?;

        let mut tty_out = pair
            .master
            .as_raw_fd()
            .map(|fd| unsafe { tokio::fs::File::from_raw_fd(fd) })
            .ok_or(eyre!("Failed to get master fd"))?;

        // Send data to the pty by writing to the master
        let mut tty_in = pair
            .master
            .take_writer()
            .map_err(|e| eyre!(format!("{:?}", e)))?;

        const BUF_SIZE: usize = 4096;
        let mut buf = [0_u8; BUF_SIZE];
        loop {
            tokio::select! {
                action = action_rx.recv() => {
                    Self::handle_action(action, &event_tx, &mut tty_in).await?;
                },
                output = tty_out.read(&mut buf) => {
                    Self::handle_output(&mut event_tx, output, &buf).await?;
                }
            };
        }
        Ok(())
    }
}

impl Component for Pty {
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
                let task = Self::start_pty(event_rx, action_tx);
                let handle = tokio::spawn(task);
                Ok(Some(handle))
            }
            _ => Err(eyre!("Failed to get event or action channel")),
        }
    }
    fn handle_events(&mut self, event: &event::Event) -> Result<Vec<action::Action>> {
        let mut ret = vec![];
        match event {
            event::Event::Pty(self::Event::PtyIn(data)) => {
                ret.push(action::Action::Pty(Action::PtyIn(data.clone())));
            }
            _ => {}
        };
        Ok(ret)
    }
}
