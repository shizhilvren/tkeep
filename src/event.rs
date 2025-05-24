use strum::Display;

#[derive(Debug, Display)]
pub enum Event {
    Server(server::Event),
    Client(client::Event),
}

pub mod server {
    use crate::app_server;
    use crate::components::server::{client_listener, pty};
    use strum::Display;

    #[derive(Debug, Display)]
    pub enum Event {
        App(app_server::Event),
        Pty(pty::Event),
        ClinetListener(client_listener::Event),
    }
}

pub mod client {
    use crate::components::client::{input, worker};
    use strum::Display;
    #[derive(Debug, Display)]
    pub enum Event {
        Input(input::Event),
        Worker(worker::Event),
    }
}
