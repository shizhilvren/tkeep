// Debug-only helper binary. In release builds (`debug_assertions` off) this
// becomes a stub that immediately exits, so `cargo build --release` does not
// ship a functional `tclient`.

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
        let mut app = tkeep::app_client::AppClient::new("test".to_string())?;
        app.run().await?;
        Ok(())
    }
}

fn main() {
    #[cfg(debug_assertions)]
    {
        imp::run().expect("tclient failed");
    }
    #[cfg(not(debug_assertions))]
    {
        eprintln!("tclient is a debug-only helper; not available in release builds");
        std::process::exit(1);
    }
}
