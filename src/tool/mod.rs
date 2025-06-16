pub const BUF_SIZE: usize = 4096;
pub const TTY_SIZE: (u16, u16, u16, u16) = (24, 80, 0, 0);
pub const TKEEP_SERVER_PTY_NAME : &str = "TKEEP_SERVER_PTY_NAME";

pub mod unix_socket {
    use bincode::{self, Decode, Encode};
    use bytes::{Buf, BytesMut};
    use color_eyre::{Result, eyre::eyre};
    use std::fmt::Debug;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;
    use tracing::{debug, trace};

    #[derive(Debug)]
    pub struct MessageReader {
        buf: BytesMut,
        len: Option<u64>,
    }
    impl MessageReader {
        pub fn new() -> MessageReader {
            Self {
                buf: BytesMut::new(),
                len: None,
            }
        }

        pub async fn receive_message<T>(&mut self, uds: &mut UnixStream) -> Result<T>
        where
            T: Decode<()> + Debug,
        {
            const SIZE_OF_U64: usize = std::mem::size_of::<u64>();
            let mut buf = [0_u8; super::BUF_SIZE];
            let len = match self.len {
                Some(len) => len,
                None => {
                    while self.buf.len() < SIZE_OF_U64 {
                        let n = uds.read(&mut buf).await?;
                        if n == 0 {
                            return Err(eyre!("uds was colse"));
                        }
                        trace!("get {} bytes from uds,", &n);
                        self.buf.extend_from_slice(&buf[..n]);
                    }
                    let len = self.buf.get_u64_le();
                    self.len = Some(len);
                    len
                }
            };
            let len = len as usize;
            while self.buf.len() < len {
                let n = uds.read(&mut buf).await?;
                if n == 0 {
                    return Err(eyre!("uds was colse"));
                }
                self.buf.extend_from_slice(&buf[..n]);
            }
            let data = &self.buf[0..len];
            let msg: (T, usize) = bincode::decode_from_slice(&data, bincode::config::standard())?;
            if msg.1 != len as usize {
                return Err(eyre!(
                    "Data length mismatch: expected {}, got {}",
                    len,
                    msg.1
                ));
            }
            // debug!("{:?}", &self);
            self.buf.advance(len);
            self.len = None;
            debug!("recv msg {:?}", &msg);
            Ok(msg.0)
        }
    }

    pub async fn send_message<T>(stream: &mut UnixStream, msg: &T) -> Result<()>
    where
        T: Encode + Debug,
    {
        // 序列化数据
        let data = bincode::encode_to_vec(msg, bincode::config::standard())?;
        debug!("send msg data {:?}", &msg);

        // 写入长度前缀（小端序 8 字节）
        let len = data.len() as u64;

        stream.write_all(&len.to_le_bytes()).await?;
        // 写入数据体
        stream.write_all(&data).await?;
        // stream.flush().await?;
        Ok(())
    }
}
