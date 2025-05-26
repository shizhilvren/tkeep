use color_eyre::Result;
use tracing::debug;
mod action;
mod app_client;
mod app_server;
mod components;
mod config;
mod errors;
mod event;
mod logging;
mod message;
mod tool;

#[tokio::main(flavor = "current_thread")]

async fn main() -> Result<()> {
    crate::errors::init()?;
    crate::logging::init()?;
    println!("Hello, world!");
    debug!("this is a debug message");
    let mut app = crate::app_server::AppServer::new()?;
    app.run().await?;
    Ok(())
}
