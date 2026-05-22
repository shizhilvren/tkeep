use clap::Parser;
use color_eyre::{Result, eyre::eyre};
use daemonize::Daemonize;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;
use std::{env, fs::File};
use tkeep::app_client::AppClient;
use tkeep::app_server::AppServer;
use tkeep::cli::{self, Cli};
use tkeep::{app_ls, tool};
use tracing::debug;

fn main() -> Result<()> {
    tkeep::errors::init()?;
    tkeep::logging::init()?;

    let args = Cli::parse();
    debug!("tkeep args are {:?}", &args);

    match args.command {
        cli::CliSubCommand::New(server_args) => {
            start_server(
                server_args.name,
                server_args.shell,
                server_args.history,
                server_args.no_attach,
            )?;
        }
        cli::CliSubCommand::Attach(client_args) => {
            start_client(client_args.name)?;
        }
        cli::CliSubCommand::Ls => {
            ls_sesssions()?;
        }
    }
    Ok(())
}

fn start_server(name: String, shell: String, history: u32, no_attach: bool) -> Result<()> {
    println!("Starting server with name: {}, shell: {}", name, shell);
    let stdout = File::create(tool::CFG.stdout(&name))?;
    let stderr = File::create(tool::CFG.stderr(&name))?;
    let (mut ready_reader, mut ready_writer) = UnixStream::pair()?;
    let client_name = name.clone();

    let daemonize = Daemonize::new()
        .pid_file(tool::CFG.pid(&name)) // Every method except `new` and `start`
        .working_directory(env::current_dir()?) // for default behaviour.
        .stdout(stdout)
        .stderr(stderr)
        .privileged_action(move || -> Result<(tokio::runtime::Runtime, AppServer)> {
            println!("privileged_action.");
            let server_app = AppServer::new(name.clone(), shell, history)?;
            let server_app = server_app.init()?;
            let run_time = tokio::runtime::Builder::new_current_thread()
                // .worker_threads(2)
                .enable_all()
                .build()?;
            Ok((run_time, server_app))
        });
    match daemonize.execute() {
        daemonize::Outcome::Child(ans) => {
            drop(ready_reader);
            match ans {
                Ok(ans) => {
                    println!("daemonize start.");
                    match ans.privileged_action_result {
                        Ok((run_time, mut server_app)) => {
                            ready_writer
                                .write_all(&[1])
                                .map_err(|e| eyre!("Error notifying parent: {}", e))?;
                            run_time.block_on(async {
                                println!("server_app start.");
                                server_app.run().await?;
                                Ok(())
                            })
                        }
                        Err(e) => Err(eyre!("Error, {}", e)),
                    }
                }
                Err(e) => {
                    ready_writer
                        .write_all(&[0])
                        .map_err(|e| eyre!("Error notifying parent: {}", e))?;
                    Err(eyre!(
                        "'{}' was already started. Please use 'tkeep attach' to connect to it directly. {}",
                        client_name,
                        e
                    ))
                }
            }
        }
        daemonize::Outcome::Parent(ans) => match ans {
            Ok(_) => {
                drop(ready_writer);
                ready_reader.set_read_timeout(Some(Duration::from_secs(5)))?;

                let mut flag = [0_u8; 1];
                match ready_reader.read(&mut flag) {
                    Ok(1) if flag[0] == 1 => {
                        if !no_attach {
                            start_client(client_name)?;
                        }
                        Ok(())
                    }
                    Ok(0) => Err(eyre!(
                        "Error start client, child exited before reporting successful startup"
                    )),
                    Ok(_) => Ok(()),
                    Err(e) => Err(eyre!("Error daemonize, waiting child ready failed: {}", e)),
                }
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

fn ls_sesssions() -> Result<()> {
    let mut sessions = app_ls::get_all_sessions(&tool::CFG.dir())?;
    sessions.sort();
    if sessions.is_empty() {
        println!("No active sessions found.");
    } else {
        println!("Active sessions:");
        for session in sessions {
            println!("- {}", session);
        }
    }
    Ok(())
}
