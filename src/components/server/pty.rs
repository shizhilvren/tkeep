use super::super::Component;
use crate::action::Action::Server as s_action_e;
use crate::action::server::Action as s_action;
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::tool;
use crate::{action, event};
use color_eyre::{Result, eyre::eyre};
use portable_pty::{CommandBuilder, ExitStatus, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::{option::Option, str::FromStr};
use strum::Display;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    PtyFinish,
    PtyIn(Vec<u8>),
    Resize { width: u16, height: u16 },
}

#[derive(Debug, Clone, Display)]
pub enum Event {
    PtyFinish(ExitStatus),
    PtyOut(Vec<u8>),
    PtyIn(Vec<u8>),
    Resize { width: u16, height: u16 },
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
        _event_tx: &UnboundedSender<event::Event>,
        tty_in: &mut Box<dyn std::io::Write + Send + 'static>,
        pty_pair: &mut portable_pty::PtyPair,
    ) -> Result<bool> {
        match action {
            Some(action) => match action {
                s_action_e(s_action::Pty(self::Action::PtyIn(data))) => {
                    if let Err(e) = tty_in.write_all(&data) {
                        error!("Failed to write to pty: {}", e);
                        return Err(eyre!("Failed to write to pty"));
                    }
                }
                s_action_e(s_action::Pty(self::Action::Resize { width, height })) => {
                    pty_pair
                        .master
                        .resize(PtySize {
                            rows: height,
                            cols: width,
                            pixel_width: 0,
                            pixel_height: 0,
                        })
                        .map_err(|e| eyre!("Failed to resize pty: {:?}", e))?;
                }
                s_action_e(s_action::Pty(Action::PtyFinish)) => {
                    return Ok(true);
                }
                _ => {}
            },
            None => {
                error!("Failed to receive action");
                return Err(eyre!("Failed to receive action"));
            }
        }
        Ok(false)
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
                event_tx.send(s_event_e(s_event::Pty(Event::PtyOut(data))))?;
            }
            Err(e) => {
                error!("Failed to read from pty: {}", e);
                return Err(eyre!("Failed to read from pty"));
            }
        }
        Ok(())
    }
    fn await_child_stop(
        mut child: Box<dyn portable_pty::Child + Send + Sync + 'static>,
        event_tx: UnboundedSender<event::Event>,
    ) -> JoinHandle<Result<()>> {
        use color_eyre::eyre::ErrReport;

        let handle = tokio::spawn(async move {
            loop {
                let status = child.try_wait()?;
                match status {
                    Some(status) => {
                        event_tx.send(s_event_e(s_event::Pty(Event::PtyFinish(status))))?;
                        break;
                    }
                    None => {
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            debug!("waiting tty finish");
            Ok::<_, ErrReport>(())
        });
        handle
    }

    async fn start_pty(
        mut event_tx: UnboundedSender<event::Event>,
        mut action_rx: UnboundedReceiver<action::Action>,
    ) -> Result<()> {
        let pty_system = native_pty_system();
        let mut pair = pty_system
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

        let mut tty_out: tokio::fs::File = pair
            .master
            .as_raw_fd()
            .map(|fd| {
                let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) };
                fd.try_clone_to_owned().and_then(|fd| {
                    let file = std::fs::File::from(fd);
                    Ok(tokio::fs::File::from_std(file))
                })
            })
            .ok_or(eyre!("Failed to get master fd"))??;

        // Send data to the pty by writing to the master
        let mut tty_in = pair
            .master
            .take_writer()
            .map_err(|e| eyre!(format!("{:?}", e)))?;

        let mut buf = [0_u8; tool::BUF_SIZE];
        let _child_status = Self::await_child_stop(child, event_tx.clone());
        loop {
            tokio::select! {
                action = action_rx.recv() => {
                    let break_flag = Self::handle_action(action, &event_tx, &mut tty_in, &mut pair).await?;
                    if break_flag{
                        break;
                    }
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
            s_event_e(s_event::Pty(self::Event::PtyIn(data))) => {
                ret.push(s_action_e(s_action::Pty(Action::PtyIn(data.clone()))));
            }
            s_event_e(s_event::Pty(self::Event::Resize { width, height })) => {
                ret.push(s_action_e(s_action::Pty(Action::Resize {
                    width: *width,
                    height: *height,
                })));
            }
            s_event_e(s_event::Pty(Event::PtyFinish(s))) => {
                debug!("pty finish as {:}", &s);
                ret.push(s_action_e(s_action::Pty(Action::PtyFinish)));
            }
            _ => {}
        };
        Ok(ret)
    }
    fn action_filter(&mut self, action: &action::Action) -> bool {
        match action {
            s_action_e(s_action::Pty(_)) => true,
            _ => false,
        }
    }
}
