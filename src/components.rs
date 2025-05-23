use color_eyre::Result;

use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};
// use tracing::debug;

use crate::{
    action::{self, Action},
    config::Config,
    event::Event,
};

pub mod clinetlistener;
pub mod clinetworker;
pub mod pty;

/// `Component` is a trait that represents a visual and interactive element of the user interface.
///
/// Implementors of this trait can be registered with the main application loop and will be able to
/// receive events, update state, and be rendered on the screen.
pub trait Component {
    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        let _ = config; // to appease clippy
        Ok(())
    }

    fn register_action_handler(&mut self) -> Result<Option<UnboundedSender<Action>>> {
        Ok(None)
    }

    fn register_event_handler(&mut self, tx: UnboundedSender<Event>) -> Result<()> {
        let _ = tx; // to appease clippy
        Ok(())
    }

    fn run(&mut self) -> Result<Option<JoinHandle<Result<()>>>> {
        Ok(None)
    }

    fn handle_events(&mut self, event: &Event) -> Result<Vec<Action>> {
        Ok(vec![])
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        let _ = action; // to appease clippy
        Ok(())
    }
}
