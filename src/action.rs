use crate::components::server::pty;
use serde::{Deserialize, Serialize};
use strum::Display;
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    ServerStart,
    Pty(pty::Action),
}
