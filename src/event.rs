use serde::{Deserialize, Serialize};
use strum::Display;

use crate::app::{self};

#[derive(Debug, PartialEq, Eq, Display)]
pub enum Event {
    App(app::Event),
}
