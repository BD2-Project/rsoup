//! Transporte de frames del driver hacia el gestor.
//!
//! El transporte por defecto es **TCP plano** (`TcpTransport`), que reproduce
//! el comportamiento histórico del driver. El transporte **TLS** es opt-in:
//! vive detrás de la feature cargo `tls` y no cambia el default.
//!
//! Nota: el gestor SoupDB todavía no implementa handshake TLS (está declarado
//! como trabajo futuro en su plan). Esta abstracción deja el driver listo para
//! cuando el servidor lo soporte, sin romper la conexión actual.

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol::{Frame, HEADER_SIZE, MAGIC, VERSION};

async fn write_frame<S>(stream: &mut S, frame: &Frame) -> io::Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    let bytes = frame.encode();
    stream.write_all(&bytes).await?;
    stream.flush().await
}

async fn read_frame<S>(stream: &mut S) -> io::Result<Frame>
where
    S: AsyncReadExt + Unpin,
{
    let mut header = [0u8; HEADER_SIZE];
    stream.read_exact(&mut header).await?;
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
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).await?;
    Ok(Frame { opcode, payload })
}

/// Transporte TCP plano (comportamiento por defecto).
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Abre una conexión TCP al gestor (host, puerto del pool).
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let stream = TcpStream::connect((host, port)).await?;
        Ok(Self { stream })
    }

    /// Envía un frame completo al gestor.
    pub async fn send_frame(&mut self, frame: &Frame) -> io::Result<()> {
        write_frame(&mut self.stream, frame).await
    }

    /// Lee exactamente un frame del gestor.
    pub async fn recv_frame(&mut self) -> io::Result<Frame> {
        read_frame(&mut self.stream).await
    }
}

/// Transporte TLS opt-in (feature `tls`).
#[cfg(feature = "tls")]
pub struct TlsTransport {
    stream: tokio_rustls::client::TlsStream<TcpStream>,
}

#[cfg(feature = "tls")]
impl TlsTransport {
    /// Abre una conexión TLS al gestor con la configuración de cliente dada.
    pub async fn connect(
        host: &str,
        port: u16,
        config: tokio_rustls::rustls::ClientConfig,
    ) -> io::Result<Self> {
        let tcp = TcpStream::connect((host, port)).await?;
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
        let server_name = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_string())
            .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid TLS server name: {error}"),
            )
        })?;
        let stream = connector.connect(server_name, tcp).await?;
        Ok(Self { stream })
    }

    pub async fn send_frame(&mut self, frame: &Frame) -> io::Result<()> {
        write_frame(&mut self.stream, frame).await
    }

    pub async fn recv_frame(&mut self) -> io::Result<Frame> {
        read_frame(&mut self.stream).await
    }
}

/// Configuración TLS por defecto usando las raíces públicas de `webpki-roots`.
#[cfg(feature = "tls")]
pub fn default_tls_config() -> tokio_rustls::rustls::ClientConfig {
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};

    // Instalar el proveedor criptográfico `ring` una sola vez (idempotente).
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// Transporte del driver: TCP plano por defecto y TLS opt-in.
pub enum Transport {
    Tcp(TcpTransport),
    #[cfg(feature = "tls")]
    Tls(TlsTransport),
}

impl Transport {
    /// Conecta por TCP plano (default).
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        Ok(Self::Tcp(TcpTransport::connect(host, port).await?))
    }

    /// Conecta por TLS (requiere la feature `tls`).
    #[cfg(feature = "tls")]
    pub async fn connect_tls(
        host: &str,
        port: u16,
        config: tokio_rustls::rustls::ClientConfig,
    ) -> io::Result<Self> {
        Ok(Self::Tls(TlsTransport::connect(host, port, config).await?))
    }

    /// Envía un frame por el transporte activo.
    pub async fn send_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match self {
            Self::Tcp(inner) => inner.send_frame(frame).await,
            #[cfg(feature = "tls")]
            Self::Tls(inner) => inner.send_frame(frame).await,
        }
    }

    /// Lee un frame del transporte activo.
    pub async fn recv_frame(&mut self) -> io::Result<Frame> {
        match self {
            Self::Tcp(inner) => inner.recv_frame().await,
            #[cfg(feature = "tls")]
            Self::Tls(inner) => inner.recv_frame().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{encode_query, OP_PING, OP_QUERY};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn tcp_transport_send_and_recv_frame() {
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

        let mut transport = TcpTransport::connect("127.0.0.1", addr.port())
            .await
            .unwrap();
        transport
            .send_frame(&Frame {
                opcode: OP_QUERY,
                payload: encode_query("SELECT 1"),
            })
            .await
            .unwrap();
        let frame = transport.recv_frame().await.unwrap();
        assert_eq!(frame.opcode, OP_PING);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn transport_default_is_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
        });
        let transport = Transport::connect("127.0.0.1", addr.port()).await.unwrap();
        assert!(matches!(transport, Transport::Tcp(_)));
        server.await.unwrap();
    }
}
