//! Compatibilidad: el transporte TCP vive ahora en [`crate::transport`].
//!
//! Se conserva este módulo para no romper imports históricos de
//! `rsoup::tcp_server::Socket`. El código nuevo debe usar
//! [`crate::transport::Transport`].

pub use crate::transport::TcpTransport;

/// Alias histórico del transporte TCP.
pub type Socket = TcpTransport;
