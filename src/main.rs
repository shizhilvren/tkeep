use clap::Parser;
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
use crate::app_client::AppClient;
use crate::app_server::AppServer;
use cli::Cli;
use color_eyre::{Result, eyre::eyre};
use daemonize::Daemonize;
use std::{env, fs::File};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    crate::errors::init()?;
    crate::logging::init()?;

    let args = Cli::parse();
    debug!("tkeep args are {:?}", &args);
    match args.command {
        cli::CliSubCommand::New(server_args) => {
            start_server(server_args.name).await?;
        }
        cli::CliSubCommand::Attach(client_args) => {
            let mut client_app = AppClient::new(client_args.name)?;
            client_app.run().await?;
        }
    }
    Ok(())
}

pub async fn start_server(name: String) -> Result<()> {
    let stdout = File::create(format!("/tmp/{}.tkeep.out", &name))?;
    let stderr = File::create(format!("/tmp/{}.tkeep.err", &name))?;
    let daemonize = Daemonize::new()
        .pid_file(format!("/tmp/{}.tkeep.pid", &name)) // Every method except `new` and `start`
        .working_directory(env::current_dir()?) // for default behaviour.
        .stdout(stdout) // Redirect stdout to `/tmp/daemon.out`.
        .stderr(stderr) // Redirect stderr to `/tmp/daemon.err`.
        .privileged_action(|| "Executed before drop privileges");

    match daemonize.start() {
        Ok(_) => {
            let mut server_app = AppServer::new(name)?;
            server_app.run().await?;
            Ok(())
        }
        Err(e) => Err(eyre!("Error, {}", e)),
    }
}
