use serde::{Deserialize, Serialize};
use strum::Display;

use crate::app::{self};
use crate::components::clinetlistener;
use crate::components::pty;

#[derive(Debug, Display)]
pub enum Event {
    App(app::Event),
    Pty(pty::Event),
    ClinetListener(clinetlistener::Event),
}