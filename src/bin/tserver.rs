use color_eyre::Result;
use tracing::debug;

#[tokio::main(flavor = "current_thread")]

async fn main() -> Result<()> {
    tkeep::errors::init()?;
    tkeep::logging::init()?;
    println!("Hello, world!");
    debug!("this is a debug message");
    let mut app = tkeep::app_server::AppServer::new("test".to_string())?;
    app.run().await?;
    Ok(())
}
