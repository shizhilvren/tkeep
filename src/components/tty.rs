use super::Component;
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;
use std::option::Option;
use crate::action;

#[derive(Debug, Clone, Default)]
pub struct TTy {
    action_tx: Option<UnboundedSender<action::Action>>,
}

impl TTy {
    pub fn new() -> Self {
        TTy {}
    }
}

impl Component for TTy {
    fn register_action_handler(
        &mut self,
        tx: UnboundedSender<crate::action::Action>,
    ) -> Result<()> {
        Ok(())
    }
}
