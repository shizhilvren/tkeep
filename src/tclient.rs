use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
mod message;
mod tool;
use crate::message::Msg;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect("/tmp/stream.sock").await?;
    tool::unix_socket::send_message(&mut stream, &Msg::PtyIn("stty size\n".as_bytes().to_vec()))
        .await?;
    Ok(())
}
