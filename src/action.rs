use serde::{Deserialize, Serialize};
use strum::Display;
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Server(server::Action),
    Clinet(client::Action),
}

pub mod server {
    use crate::components::server::{pty, pty_buffer};
    use serde::{Deserialize, Serialize};
    use strum::Display;

    #[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
    pub enum Action {
        ServerStart,
        Pty(pty::Action),
        PtyBuffer(pty_buffer::Action),
    }
}

pub mod client {
    use serde::{Deserialize, Serialize};
    use strum::Display;

    use crate::components::client::worker;

    #[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
    pub enum Action {
        Worker(worker::Action),
    }
}
