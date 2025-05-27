use std::collections::HashMap;

use crate::action;
use crate::action::Action::Clinet as c_action_e;
use crate::action::client::Action as c_action;
use crate::components::client::output;
use crate::components::Component;
use crate::components::client::input;
use crate::components::client::worker;
use crate::config;
use crate::event;
use crate::event::Event::Client as c_event_e;
use crate::event::client::Event as c_event;
use color_eyre::Result;
use config::Config;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

pub struct AppClient {
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    StartPty,
    StartClinetListener,
}

impl AppClient {
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
                                let actions = component.handle_events(&event);
                                match actions {
                                    Ok(actions) => acc.extend(actions),
                                    Err(e) => {
                                        error!("Failed to handle event {:?} in component {}: {}", event, id, e);
                                    }
                                }
                                acc
                            },
                        );

                        actions.iter().for_each(|action| {
                            debug!("Received action {:?}", action);
                            self.components.iter_mut().for_each(
                                 |(id, (sander, _, component))| {
                                    match (sander, component.action_filter(action)) {
                                        (Some(sander), true) => match sander.send(action.clone()) {
                                            Err(e) => error!(
                                                "Failed to send action {:?} to component {}: {}",
                                                action, id, e
                                            ),
                                            _ => {}
                                        },
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
            c_event_e(c_event::Input(input::Event::Start)) => {
                self.add_component(Box::new(input::Input::new()))?;
                self.add_component(Box::new(output::Output::new()))?;
                None
            }
            c_event_e(c_event::Worker(worker::Event::Start)) => {
                self.add_component(Box::new(worker::Worker::new()))?;
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
        debug!("start client");
        self.event_tx
            .send(c_event_e(c_event::Worker(worker::Event::Start)))?;
        self.event_tx
            .send(c_event_e(c_event::Input(input::Event::Start)))?;
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
