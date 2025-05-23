pub mod unix_socket {
    use bincode::{self, Decode, Encode};
    use color_eyre::{Result, eyre::eyre};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    pub async fn send_message<T: Encode>(stream: &mut UnixStream, msg: &T) -> Result<()> {
        // 序列化数据
        let data = bincode::encode_to_vec(msg, bincode::config::standard())?;

        // 写入长度前缀（小端序 8 字节）
        let len = data.len() as u64;
        stream.write_all(&len.to_le_bytes()).await?;

        // 写入数据体
        stream.write_all(&data).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn receive_message<T: Decode<()>>(stream: &mut UnixStream) -> Result<T> {
        // 读取长度前缀
        let mut len_buf = [0u8; size_of::<u64>()];
        stream.read_exact(&mut len_buf).await?;
        let len = u64::from_le_bytes(len_buf) as usize;

        // 读取数据体
        let mut data_buf = vec![0u8; len];
        stream.read_exact(&mut data_buf).await?;

        // 反序列化数据
        let msg: (T, usize) =
            bincode::decode_from_slice(&data_buf.as_mut_slice(), bincode::config::standard())?;
        match msg.1 == len {
            true => {}
            false => {
                return Err(eyre!(
                    "Data length mismatch: expected {}, got {}",
                    len,
                    msg.1
                ));
            }
        }
        Ok(msg.0)
    }
}
