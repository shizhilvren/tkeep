// Debug-only helper binary. In release builds (`debug_assertions` off) this
// becomes a stub that immediately exits, so `cargo build --release` does not
// ship a functional `tserver`.

#[cfg(debug_assertions)]
mod imp {
    use color_eyre::Result;
    use tracing::debug;

    #[tokio::main(flavor = "current_thread")]
    pub async fn run() -> Result<()> {
        tkeep::errors::init()?;
        tkeep::logging::init()?;
        println!("Hello, world!");
        debug!("this is a debug message");
        let mut app = tkeep::app_server::AppServer::new(
            "test".to_string(),
            "bash".to_string(),
            1000,
        )?;
        app.run().await?;
        Ok(())
    }
}

fn main() {
    #[cfg(debug_assertions)]
    {
        imp::run().expect("tserver failed");
    }
    #[cfg(not(debug_assertions))]
    {
        eprintln!("tserver is a debug-only helper; not available in release builds");
        std::process::exit(1);
    }
}
