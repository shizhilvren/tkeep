use std::collections::HashMap;

use crate::components::{self, clinetlistener, clinetworker, pty};
use crate::{action, components::Component, config::Config, event};
use color_eyre::Result;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

pub struct App {
    config: Config,
    components: HashMap<
        u32,
        (
            Option<UnboundedSender<action::Action>>,
            Option<JoinHandle<Result<()>>>,
            Box<dyn Component>,
        ),
    >,
    event_tx: mpsc::UnboundedSender<event::Event>,
    event_rx: mpsc::UnboundedReceiver<event::Event>,
}

use crate::app;
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    StartPty,
    StartClinetListener,
}

impl App {
    pub fn new() -> Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Ok(Self {
            components: HashMap::new(),
            event_tx,
            event_rx,
            config: Config::new()?,
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
                                let actions = component.handle_events(&event).unwrap_or_default();
                                acc.extend(actions);
                                acc
                            },
                        );

                        actions.iter().for_each(|action| {
                            self.components.iter_mut().for_each(
                                move |(id, (sander, _, component))| {
                                    let ans: std::result::Result<(), color_eyre::eyre::Error> =
                                        component.handle_action(action.clone());
                                    match ans {
                                        Err(e) => {
                                            error!("handle_action fail {:?}", e);
                                        }
                                        _ => {}
                                    };
                                    match sander {
                                        Some(sander) => {
                                            sander.send(action.clone());
                                        }
                                        _ => {}
                                    };
                                },
                            );
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
            event::Event::App(app::Event::StartPty) => {
                debug!("Received StartPty event");
                self.add_component(Box::new(pty::Pty::new()))?;
                None
            }
            event::Event::App(app::Event::StartClinetListener) => {
                debug!("Received StartClinetListener event");
                self.add_component(Box::new(clinetlistener::ClinetListener::new()))?;
                None
            }
            event::Event::ClinetListener(clinetlistener::Event::NewClient(stream)) => {
                self.add_component(Box::new(clinetworker::ClinetWorker::new(stream)))?;
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
            .send(event::Event::App(app::Event::StartPty))?;
        self.event_tx
            .send(event::Event::App(app::Event::StartClinetListener))?;
        debug!("Sent StartPty event");
        Ok(())
    }

    fn get_unused_id(&self) -> u32 {
        let mut id = 0;
        while self.components.contains_key(&id) {
            id += 1;
        }
        id
    }

    fn add_component(&mut self, component: Box<dyn Component>) -> Result<u32> {
        debug!("Adding component");
        let id = self.get_unused_id();
        self.components.insert(id, (None, None, component));
        match self.components.get_mut(&id) {
            Some((sender, task, component)) => {
                component.init()?;
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
