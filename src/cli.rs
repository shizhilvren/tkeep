use crate::config::{get_config_dir, get_data_dir};
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::builder::styling::{self, AnsiColor};

const STYLES: styling::Styles = styling::Styles::styled()
    .header(AnsiColor::Yellow.on_default())
    .usage(AnsiColor::Green.on_default())
    .literal(AnsiColor::Green.on_default())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Parser, Debug)]
#[command(author, version = version(), about, styles=STYLES)]
pub struct Cli {
    // /// Set which gdb debugger to use.
    // #[arg(short('s'), long, value_name = "SESSION")]
    // pub session: String,
    #[command(subcommand)]
    pub command: CliSubCommand,
}

#[derive(Subcommand, Debug)]
#[command(
    about = "tkeep is a terminal keep alive tool, it can keep your terminal session alive even if you close the terminal window.",
    args_conflicts_with_subcommands = true
)]
pub enum CliSubCommand {
    /// Create a new terminal session
    #[command(visible_alias = "n")]
    New(ServerCli),

    /// Attach to an existing terminal session
    #[command(visible_alias = "a")]
    Attach(ClientCli),
}

/// Create a new terminal session
#[derive(Args, Debug)]
pub struct ServerCli {
    /// Name of the new terminal session
    #[arg(index = 1, value_name = "NAME", required = true)]
    pub name: String,

    /// Set which gdb debugger to use.
    #[arg(short('s'), long, value_name = "SHELL", default_value_t = String::from("bash"), value_parser = shell_check)]
    pub shell: String,

    /// Args will pass to gdb append "--args", if you pass "--args <some options>" to this command, it will pass same one to gdb. Note: it cannoot use with "--"
    #[arg(long, value_name = "ARGS", num_args(2..), allow_hyphen_values(true))]
    pub args: Vec<String>,

    /// set history size
    #[arg(long, value_name = "SIZE", default_value_t = 10000_u32, value_parser = clap::value_parser!(u32).range(1000..1000000))]
    pub history: u32,

    /// only start server, do not attach it
    #[arg(long)]
    pub no_attach: bool,
    // /// Args pass to gdb which not change
    // #[arg(value_name = "GDB_ARGS", last(true), num_args(2..), allow_hyphen_values(true))]
    // gdb_args: Vec<String>,
}

/// Attach to an existing terminal session
#[derive(Args, Debug)]
pub struct ClientCli {
    /// Vame of session to attach
    #[arg(index = 1, value_name = "NAME", required = true)]
    pub name: String,

    /// set is attach need replay log
    #[arg(short('r'), long, default_value_t = true)]
    pub replay: bool,
}

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_DESCRIBE"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

pub fn version() -> String {
    let author = clap::crate_authors!();

    // let current_exe_path = PathBuf::from(clap::crate_name!()).display().to_string();
    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{VERSION_MESSAGE}

Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}"
    )
}

fn shell_check(s: &str) -> Result<String, String> {
    let shell = which::which(s).map_err(|e| e.to_string())?;
    let shell = shell.into_os_string().into_string().map_err(|e| {
        e.to_str()
            .expect(format!("{} path error", &s).as_str())
            .to_string()
    })?;
    Ok(shell)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_args() {
        // let cli = Cli::try_parse_from(["rgdb", "-d", "gdb"]).unwrap();
        // assert!(cli.tick_rate == 4_f64);
        // assert!(cli.frame_rate == 24_f64);
        // assert!(cli.gdb == "/usr/bin/gdb");
    }
}
