use color_eyre::Result;
use std::env;

use crate::tool;

pub fn run() -> Result<()> {
    match env::var(tool::TKEEP_SERVER_PTY_NAME) {
        Ok(name) if !name.is_empty() => {
            println!("You are inside tkeep session: {}", name);
        }
        _ => {
            println!("You are not inside a tkeep session.");
        }
    }
    Ok(())
}
