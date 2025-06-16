use clap::Parser;
use color_eyre::{Result, eyre::eyre};
use daemonize::Daemonize;
use std::{env, fs::File};
use tkeep::app_client::AppClient;
use tkeep::app_server::AppServer;
use tkeep::cli::{self, Cli};
use tracing::debug;

fn main() -> Result<()> {
    tkeep::errors::init()?;
    tkeep::logging::init()?;

    let args = Cli::parse();
    debug!("tkeep args are {:?}", &args);

    match args.command {
        cli::CliSubCommand::New(server_args) => {
            start_server(server_args.name, server_args.shell, server_args.history)?;
        }
        cli::CliSubCommand::Attach(client_args) => {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(3)
                .enable_all()
                .build()?
                .block_on(async {
                    let mut client_app = AppClient::new(client_args.name)?;
                    match client_app.run().await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(eyre!("Error running client: {}", e)),
                    }
                })?;
        }
    }
    Ok(())
}

fn start_server(name: String, shell: String, history: u32) -> Result<()> {
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
            let run_time = tokio::runtime::Builder::new_current_thread()
                // .worker_threads(2)
                .enable_all()
                .build()?;
            run_time.block_on(async {
                let mut server_app = AppServer::new(name, shell, history)?;
                server_app.run().await?;
                Ok(())
            })
        }
        Err(e) => Err(eyre!("Error, {}", e)),
    }
}
