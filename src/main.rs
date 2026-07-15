use clap::Parser;
use color_eyre::Result;
use tkeep::cli::{self, Cli};
use tkeep::commands;
use tracing::debug;

fn main() -> Result<()> {
    tkeep::errors::init()?;
    tkeep::logging::init()?;

    let args = Cli::parse();
    debug!("tkeep args are {:?}", &args);

    match args.command {
        cli::CliSubCommand::New(server_args) => commands::new::run(
            server_args.name,
            server_args.shell,
            server_args.history,
            server_args.no_attach,
        )?,
        cli::CliSubCommand::Attach(client_args) => commands::attach::run(client_args.name)?,
        cli::CliSubCommand::Ls => commands::ls::run()?,
        cli::CliSubCommand::Where => commands::where_cmd::run()?,
        cli::CliSubCommand::Kill(kill_args) => commands::kill::run(kill_args.name, kill_args.force)?,
    }
    Ok(())
}
