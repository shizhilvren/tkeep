use clap::Parser;
use color_eyre::Result;
use tokio::task;
use tracing::debug;

mod config;
mod errors;
mod logging;
mod app;
mod action;
mod event;
mod components;

#[tokio::main(flavor = "current_thread")]

async fn main() -> Result<()> {
    crate::errors::init()?;
    crate::logging::init()?;
    println!("Hello, world!");
    debug!("this is a debug message");
    let app = crate::app::App::new()?;
    Ok(())
}
