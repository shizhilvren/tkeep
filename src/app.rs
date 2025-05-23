use crate::{action, components::Component, config::Config, event};
use color_eyre::Result;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};
use crate::components::tty;
pub struct App {
    config: Config,
    components: Vec<Box<dyn Component>>,
    action_tx: mpsc::UnboundedSender<action::Action>,
    action_rx: mpsc::UnboundedReceiver<action::Action>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        Ok(Self {
            components: vec![Box::new(tty::TTy::new())],
            action_tx,
            action_rx,
            config: Config::new()?,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        for component in self.components.iter_mut() {
            component.register_action_handler(self.action_tx.clone())?;
        }
        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }
        for component in self.components.iter_mut() {
            component.init()?;
        }

        self.init()?;

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui)?;
        }
        Ok(())
    }

    async fn handle_events(&mut self, event: event::Event) -> Result<()> {
        Ok(())
    }

    fn handle_actions(&mut self, action:action::Action) -> Result<()> {
        Ok(())
    }

    fn init(&self) -> Result<()> {
        Ok(())
    }
}
