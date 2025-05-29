use clap::Parser;
use color_eyre::Result;
use tracing::debug;
mod action;
mod app_client;
mod app_server;
mod cli;
mod components;
mod config;
mod errors;
mod event;
mod logging;
mod message;
mod tool;
use crate::{app_client::AppClient, app_server::AppServer};
use cli::Cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    crate::errors::init()?;
    crate::logging::init()?;

    let args = Cli::parse();
    debug!("tkeep args are {:?}", &args);
    match args.command {
        cli::CliSubCommand::New(server_args) => {
            let mut server_app = AppServer::new(server_args.name)?;
            server_app.run().await?;
        }
        cli::CliSubCommand::Attach(client_args) => {
            let mut client_app = AppClient::new(client_args.name)?;
            client_app.run().await?;
        }
    }
    Ok(())
}
