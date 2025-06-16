use crate::components::server::{client_listener, pty, pty_buffer, worker};
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::tool;
use crate::{
    action,
    components::{Component, PID},
    config::Config,
    event,
};
use color_eyre::Result;
use color_eyre::eyre::eyre;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
#[allow(unused_imports)]
use tracing::{debug, error, info};

pub struct AppServer {
    name: String,
    config: Config,
    shell: String,
    history: u32,
    components: HashMap<
        PID,
        (
            Option<UnboundedSender<action::Action>>,
            Option<JoinHandle<Result<()>>>,
            Box<dyn Component>,
        ),
    >,
    event_tx: mpsc::UnboundedSender<event::Event>,
    event_rx: mpsc::UnboundedReceiver<event::Event>,
}

use crate::app_server;
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    StartPty { shell: String, name: String },
    StartPtyBuffer { history: u32 },
    StartClinetListener(String),
}

impl AppServer {
    pub fn new(name: String, shell: String, history: u32) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Ok(Self {
            components: HashMap::new(),
            event_tx,
            event_rx,
            config: Config::new()?,
            name,
            shell,
            history,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.init()?;
        loop {
            let event = self.event_rx.recv().await;
            debug!("Received event: {:?}", event);
            match event {
                Some(event) => {
                    let (flag, event) = self.handle_events(event).await?;
                    if let Some(event) = event {
                        let actions = self.components.iter_mut().fold(
                            vec![],
                            |mut acc, (id, (_, _, component))| {
                                let actions = component.handle_events(&event);
                                match actions {
                                    Ok(actions) => acc.extend(actions),
                                    Err(e) => {
                                        error!(
                                            "Failed to handle event {:?} in component {:?}: {}",
                                            event, id, e
                                        );
                                    }
                                }
                                acc
                            },
                        );

                        actions.iter().for_each(|action| {
                            debug!("Received action {:?}", action);
                            self.components
                                .iter_mut()
                                .for_each(|(id, (sander, _, component))| {
                                    match (sander, component.action_filter(action)) {
                                        (Some(sander), true) => match sander.send(action.clone()) {
                                            Err(e) => error!(
                                                "Failed to send action {:?} to component {:?}: {}",
                                                action, id, e
                                            ),
                                            _ => {}
                                        },
                                        _ => {}
                                    };
                                });
                        });
                    }
                    if flag {
                        break;
                    }
                }
                None => {
                    error!("Failed to receive event");
                    return Err(color_eyre::eyre::eyre!("Failed to receive event"));
                }
            }
        }
        for (id, (_, task, _)) in self.components.iter_mut() {
            if let Some(task) = task {
                task.await??;
                debug!("{:?} is finish", id);
            }
        }
        debug!("server finish");
        Ok(())
    }

    async fn handle_events(&mut self, event: event::Event) -> Result<(bool, Option<event::Event>)> {
        let next = match event {
            s_event_e(s_event::App(app_server::Event::StartPty { shell, name })) => {
                debug!("Received StartPty event");
                self.add_component(Box::new(pty::Pty::new(shell, name)))?;
                (false, None)
            }
            s_event_e(s_event::App(app_server::Event::StartClinetListener(name))) => {
                debug!("Received StartClinetListener event");
                self.add_component(Box::new(client_listener::ClinetListener::new(name)))?;
                (false, None)
            }
            s_event_e(s_event::App(app_server::Event::StartPtyBuffer { history })) => {
                let paser = pty_buffer::paser::Paser::new(tool::TTY_SIZE.0, tool::TTY_SIZE.1);
                self.add_component(Box::new(pty_buffer::PtyBuffer::new(paser, history)))?;
                (false, None)
            }
            s_event_e(s_event::ClinetListener(client_listener::Event::NewClient(stream))) => {
                self.add_component(Box::new(worker::Worker::new(stream)))?;
                (false, None)
            }
            s_event_e(s_event::Worker(worker::Event::Stop(pid))) => {
                self.remove_component_by_id(&pid)?;
                (false, None)
            }
            s_event_e(s_event::Pty(pty::Event::PtyFinish(_))) => (true, Some(event)),
            _ => (false, Some(event)),
        };

        Ok(next)
    }

    fn init(&self) -> Result<()> {
        self.event_tx
            .send(s_event_e(s_event::App(app_server::Event::StartPty {
                shell: self.shell.clone(),
                name: self.name.clone(),
            })))?;
        self.event_tx
            .send(s_event_e(s_event::App(app_server::Event::StartPtyBuffer {
                history: self.history,
            })))?;
        self.event_tx.send(s_event_e(s_event::App(
            app_server::Event::StartClinetListener(self.name.clone()),
        )))?;
        debug!("Sent StartPty event");
        Ok(())
    }

    fn get_unused_id(&self) -> PID {
        let mut id = 0;
        while self.components.contains_key(&PID(id)) {
            id += 1;
        }
        PID(id)
    }

    fn remove_component_by_id(&mut self, pid: &PID) -> Result<()> {
        self.components
            .remove_entry(pid)
            .ok_or(eyre!("remove worker PID {:?}", &pid))?;
        Ok(())
    }

    fn add_component(&mut self, component: Box<dyn Component>) -> Result<PID> {
        let id = self.get_unused_id();
        debug!("Adding component {:?}", id);
        self.components.insert(id, (None, None, component));
        match self.components.get_mut(&id) {
            Some((sender, task, component)) => {
                component.init()?;
                component.register_id(id)?;
                component.register_config_handler(self.config.clone())?;
                match sender {
                    Some(_) => {
                        error!("Failed to register action handler");
                        Err(color_eyre::eyre::eyre!("Failed to register action handler"))?;
                    }
                    None => {
                        *sender = component.register_action_handler()?;
                    }
                }
                component.register_event_handler(self.event_tx.clone())?;
                match task {
                    Some(_) => {
                        error!("Failed to register task");
                        Err(color_eyre::eyre::eyre!("Failed to register task"))?;
                    }
                    None => {
                        debug!("Starting component {:?}", id);
                        *task = component.run()?;
                    }
                }
            }
            None => {
                return Err(color_eyre::eyre::eyre!("Failed to register component"));
            }
        };
        Ok(id)
    }
}
