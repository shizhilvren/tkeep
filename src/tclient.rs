use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
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
use message::Msg;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect("/tmp/stream.sock").await?;
    tool::unix_socket::send_message(&mut stream, &Msg::PtyIn("stty size\n".as_bytes().to_vec()))
        .await?;

    Ok(())
}
