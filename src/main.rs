use clap::Parser;
use color_eyre::{Result, eyre::eyre};
use daemonize::Daemonize;
use libc::sleep;
use std::{env, fs::File};
use tkeep::app_client::AppClient;
use tkeep::app_server::AppServer;
use tkeep::cli::{self, Cli};
use tkeep::tool;
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
            start_client(client_args.name)?;
        }
    }
    Ok(())
}

fn start_server(name: String, shell: String, history: u32) -> Result<()> {
    println!("Starting server with name: {}, shell: {}", name, shell);
    let stdout = File::create(tool::CFG.stdout(&name))?;
    let stderr = File::create(tool::CFG.stderr(&name))?;
    let client_name = name.clone();

    let server_app = AppServer::new(name.clone(), shell, history)?;
    let mut server_app = server_app.init()?;
    let run_time = tokio::runtime::Builder::new_current_thread()
        // .worker_threads(2)
        .enable_all()
        .build()?;
    run_time.block_on(async { server_app.run_one().await })?;
    let daemonize = Daemonize::new()
        .pid_file(tool::CFG.pid(&name)) // Every method except `new` and `start`
        .working_directory(env::current_dir()?) // for default behaviour.
        .stdout(stdout)
        .stderr(stderr)
        .privileged_action(move || -> Result<(tokio::runtime::Runtime, AppServer)> {
            println!("privileged_action.");

            Ok((run_time, server_app))
        });
    match daemonize.execute() {
        daemonize::Outcome::Child(ans) => match ans {
            Ok(ans) => {
                println!("daemonize start.");
                match ans.privileged_action_result {
                    Ok((run_time, mut server_app)) => run_time.block_on(async {
                        println!("server_app start.");
                        server_app.run().await?;
                        Ok(())
                    }),
                    Err(e) => Err(eyre!("Error, {}", e)),
                }
            }
            Err(e) => Err(eyre!("Error {client_name} was allready start. {}", e)),
        },
        daemonize::Outcome::Parent(ans) => match ans {
            Ok(ans) => {
                println!("child exit code: {:?}\n", ans);
                // start_client(client_name)?;
                Ok(())
            }
            Err(e) => Err(eyre!("Error, {}", e)),
        },
    }
}

fn start_client(name: String) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()?
        .block_on(async {
            let mut client_app = AppClient::new(name)?;
            match client_app.run().await {
                Ok(_) => Ok(()),
                Err(e) => Err(eyre!("Error running client: {}", e)),
            }
        })?;
    Ok(())
}
