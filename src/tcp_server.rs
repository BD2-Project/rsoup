//! Gestión de sockets capa 4 (TCP) del driver.
//!
//! El driver es cliente del gestor: abre un `TcpStream` hacia el puerto del
//! pool del gestor y lee/escribe frames del protocolo v1 de forma asíncrona
//! con tokio.

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol::{Frame, ProtocolError, HEADER_SIZE, MAGIC, VERSION};

/// Socket TCP hacia el gestor.
pub struct Socket {
    stream: TcpStream,
}

impl Socket {
    /// Abre una conexión al gestor (host, puerto del pool).
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let stream = TcpStream::connect((host, port)).await?;
        Ok(Self { stream })
    }

    /// Envía un frame completo al gestor.
    pub async fn send_frame(&mut self, frame: &Frame) -> io::Result<()> {
        let bytes = frame.encode();
        self.stream.write_all(&bytes).await?;
        self.stream.flush().await
    }

    /// Lee exactamente un frame del gestor.
    pub async fn recv_frame(&mut self) -> io::Result<Frame> {
        let header = self.recv_exact(HEADER_SIZE).await?;
        if header[0..2] != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bad magic {:?}", &header[0..2]),
            ));
        }
        if header[2] != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported protocol version {}", header[2]),
            ));
        }
        let opcode = header[3];
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let payload = self.recv_exact(length).await?;
        Ok(Frame { opcode, payload })
    }

    async fn recv_exact(&mut self, size: usize) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        self.stream.read_exact(&mut buffer).await?;
        Ok(buffer)
    }

    /// Convierte un error de protocolo en un error de I/O (para la capa de red).
    pub fn protocol_error(err: ProtocolError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{encode_query, OP_PING, OP_QUERY};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn send_and_recv_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut header = [0u8; HEADER_SIZE];
            stream.read_exact(&mut header).await.unwrap();
            assert_eq!(&header[0..2], b"SP");
            let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
            let mut payload = vec![0u8; length];
            stream.read_exact(&mut payload).await.unwrap();
            // responde un PONG
            stream
                .write_all(
                    &Frame {
                        opcode: OP_PING,
                        payload: vec![],
                    }
                    .encode(),
                )
                .await
                .unwrap();
        });

        let mut socket = Socket::connect("127.0.0.1", addr.port()).await.unwrap();
        socket
            .send_frame(&Frame {
                opcode: OP_QUERY,
                payload: encode_query("SELECT 1"),
            })
            .await
            .unwrap();
        let frame = socket.recv_frame().await.unwrap();
        assert_eq!(frame.opcode, OP_PING);

        server.await.unwrap();
    }
}
