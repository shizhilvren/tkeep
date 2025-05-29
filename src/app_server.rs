use std::collections::HashMap;

use crate::components::{
    self,
    server::{client_listener, pty, pty_buffer, worker},
};
use crate::event::Event::Server as s_event_e;
use crate::event::server::Event as s_event;
use crate::{
    action,
    components::{Component, PID},
    config::Config,
    event,
};
use color_eyre::Result;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

pub struct AppServer {
    name: String,
    config: Config,
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
    StartPty,
    StartPtyBuffer,
    StartClinetListener(String),
}

impl AppServer {
    pub fn new(name: String) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Ok(Self {
            components: HashMap::new(),
            event_tx,
            event_rx,
            config: Config::new()?,
            name,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.init()?;
        loop {
            let event = self.event_rx.recv().await;
            debug!("Received event: {:?}", event);
            match event {
                Some(event) => {
                    if let Some(event) = self.handle_events(event).await? {
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
                }
                None => {
                    error!("Failed to receive event");
                    return Err(color_eyre::eyre::eyre!("Failed to receive event"));
                }
            }
        }
        Ok(())
    }

    async fn handle_events(&mut self, event: event::Event) -> Result<Option<event::Event>> {
        let next = match event {
            s_event_e(s_event::App(app_server::Event::StartPty)) => {
                debug!("Received StartPty event");
                self.add_component(Box::new(pty::Pty::new()))?;
                None
            }
            s_event_e(s_event::App(app_server::Event::StartClinetListener(name))) => {
                debug!("Received StartClinetListener event");
                self.add_component(Box::new(client_listener::ClinetListener::new(name)))?;
                None
            }
            s_event_e(s_event::App(app_server::Event::StartPtyBuffer)) => {
                self.add_component(Box::new(pty_buffer::PtyBuffer::new()))?;
                None
            }
            s_event_e(s_event::ClinetListener(client_listener::Event::NewClient(stream))) => {
                self.add_component(Box::new(worker::Worker::new(stream)))?;
                None
            }

            _ => Some(event),
        };

        Ok(next)
    }

    fn handle_actions(&mut self, action: action::Action) -> Result<()> {
        Ok(())
    }

    fn init(&self) -> Result<()> {
        self.event_tx
            .send(s_event_e(s_event::App(app_server::Event::StartPty)))?;
        self.event_tx
            .send(s_event_e(s_event::App(app_server::Event::StartPtyBuffer)))?;
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

    fn add_component(&mut self, component: Box<dyn Component>) -> Result<PID> {
        debug!("Adding component");
        let id = self.get_unused_id();
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
