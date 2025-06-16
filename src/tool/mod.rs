use lazy_static::lazy_static;

pub const BUF_SIZE: usize = 4096;
pub const TTY_SIZE: (u16, u16, u16, u16) = (24, 80, 0, 0);
pub const TKEEP_SERVER_PTY_NAME: &str = "TKEEP_SERVER_PTY_NAME";

lazy_static! {
    pub static ref CFG: config::Config = config::Config::new().expect("Failed to create config");
}

pub mod config {
    use directories::BaseDirs;
    use color_eyre::Result;
    use std::path::PathBuf;
    use lazy_static::lazy_static;

    lazy_static! {
        pub static ref TKEEP_PROJ_DIR: BaseDirs = BaseDirs::new().unwrap();
    }
    pub struct Config {
        dir: PathBuf,
    }

    impl Config {
        pub fn new() -> Result<Self> {
            let dir = TKEEP_PROJ_DIR
                .runtime_dir()
                .map_or_else(|| TKEEP_PROJ_DIR.home_dir(), |v| v)
                .to_path_buf()
                .join(env!("CARGO_PKG_NAME"))
                .join(env!("CARGO_PKG_VERSION"));
            std::fs::create_dir_all(&dir)?;
            Ok(Self { dir: dir })
        }
        pub fn stdout(&self, name: &String) -> PathBuf {
            self.dir.join(format!("{}.out", name))
        }
        pub fn stderr(&self, name: &String) -> PathBuf {
            self.dir.join(format!("{}.err", name))
        }
        pub fn sock(&self, name: &String) -> PathBuf {
            self.dir.join(format!("{}.sock", name))
        }
        pub fn pid(&self, name: &String) -> PathBuf {
            self.dir.join(format!("{}.pid", name))
        }
    }
}

pub mod unix_socket {
    use bincode::{self, Decode, Encode};
    use bytes::{Buf, BytesMut};
    use color_eyre::eyre::{eyre, Result};
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
