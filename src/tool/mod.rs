pub const BUF_SIZE: usize = 4096;

pub mod unix_socket {
    use bincode::{self, Decode, Encode};
    use bytes::{Buf, BytesMut};
    use color_eyre::{Result, eyre::eyre};
    use std::fmt::Debug;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;
    use tokio_util::codec::Decoder;
    use tracing::debug;

    #[derive(Debug,  PartialEq, Eq)]
    pub struct MessageReader<T:Decode<()> + Debug> {}

    impl<T: Decode<()> + Debug> Decoder for MessageReader<T> {
        type Item = T;
        type Error = color_eyre::Report;

        fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            const SIZE_OF_U64: usize = std::mem::size_of::<u64>();
            if buf.len() < SIZE_OF_U64 {
                return Ok(None);
            }

            let mut  len_buf = [0u8; SIZE_OF_U64];
            len_buf.copy_from_slice(&buf[..SIZE_OF_U64]);
            let len = u64::from_le_bytes(len_buf);
            let all_len = SIZE_OF_U64 + len as usize;
            if buf.len() < all_len {
                buf.reserve(all_len.saturating_sub(buf.len()));
                return Ok(None);
            }

            let data = &buf[SIZE_OF_U64..all_len];
            let msg: (T, usize) = bincode::decode_from_slice(&data, bincode::config::standard())?;
            if msg.1 != len as usize {
                return Err(eyre!(
                    "Data length mismatch: expected {}, got {}",
                    len,
                    msg.1
                ));
            }
            buf.advance(all_len);
            Ok(Some(msg.0))
        }
    }

    pub async fn send_message<T>(stream: &mut UnixStream, msg: &T) -> Result<()>
    where
        T: Encode + Debug,
    {
        // 序列化数据
        let data = bincode::encode_to_vec(msg, bincode::config::standard())?;

        // 写入长度前缀（小端序 8 字节）
        let len = data.len() as u64;
        // debug!(
        //     "send message {:?} len is {:} {:?}, data is {:?}",
        //     msg,
        //     &len,
        //     &len.to_le_bytes(),
        //     &data
        // );
        stream.write_all(&len.to_le_bytes()).await?;
        debug!("send message len {:?} {:?}", &len, &len.to_le_bytes(),);
        // 写入数据体
        stream.write_all(&data).await?;
        debug!("send message data {:?} {:?}", &data.len(), &data,);
        // stream.flush().await?;
        Ok(())
    }

    pub async fn receive_message<T>(stream: &mut UnixStream) -> Result<T>
    where
        T: Decode<()> + Debug,
    {
        // 读取长度前缀
        let mut len_buf = [0u8; size_of::<u64>()];
        stream.read_exact(&mut len_buf).await?;

        let len = u64::from_le_bytes(len_buf);
        debug!("Received message len {:?} {:?}", &len, &len_buf);
        assert_ne!(
            usize::MAX as u64,
            len,
            "Received message length exceeds usize::MAX"
        );
        let len = len as usize;

        // 读取数据体
        let mut data_buf = vec![0u8; len];
        stream.read_exact(&mut data_buf).await?;
        debug!("Received message data {:?} {:?}", &len, &data_buf);

        // 反序列化数据
        let msg: (T, usize) =
            bincode::decode_from_slice(&data_buf.as_mut_slice(), bincode::config::standard())?;
        // debug!("Received message {:?}", msg);
        match msg.1 == len {
            false => {
                return Err(eyre!(
                    "Data length mismatch: expected {}, got {}",
                    len,
                    msg.1
                ));
            }
            true => {}
        }
        Ok(msg.0)
    }
}
