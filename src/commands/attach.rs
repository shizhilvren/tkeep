use color_eyre::{Result, eyre::eyre};

use crate::app_client::AppClient;

pub fn run(name: String) -> Result<()> {
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
