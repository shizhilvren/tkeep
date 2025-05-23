use super::Component;
use color_eyre::Result;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::error;
use std::option::Option;
use crate::{action, event};

#[derive(Debug, Default)]
pub struct TTy {
    event_rx: Option<UnboundedSender<event::Event>>,
    action_tx: Option<UnboundedReceiver<action::Action>>,
}

impl TTy {
    pub fn new() -> Self {
        TTy::default()
    }
    fn run_tty
}

impl Component for TTy {
    fn register_action_handler(&mut self) -> Result<Option<UnboundedSender<action::Action>>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.action_tx = Some(rx);
        Ok(Some(tx))
    }
    fn register_event_handler(&mut self, tx: UnboundedSender<event::Event>) -> Result<()> {
        self.event_rx = Some(tx);
        Ok(())
    }
    fn run(&mut self) -> Result<Option<tokio::task::JoinHandle<Result<()>>>> {
        let event_rx = self.event_rx.clone();
        let  action_tx = self.action_tx.take();
        tokio::spawn(async move {
            let event_rx = event_rx.unwrap();
            let action_tx = action_tx.unwrap();
            loop {
                let event = event_rx.recv().await;
                match event {
                    Some(event) => {
                        // Handle the event
                        // For example, you can send an action based on the event
                        if let Err(e) = action_tx.send(action::Action::SomeAction) {
                            error!("Failed to send action: {}", e);
                        }
                    }
                    None => {
                        error!("Failed to receive event");
                        return Err(color_eyre::eyre::eyre!("Failed to receive event"));
                    }
                }
            }
        });

    }
}
