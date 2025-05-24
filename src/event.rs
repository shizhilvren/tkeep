use crate::app_server;
use crate::components::server::{client_listener, pty};
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Display)]
pub enum Event {
    App(app_server::Event),
    Pty(pty::Event),
    ClinetListener(client_listener::Event),
}
