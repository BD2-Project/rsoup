//! Driver Rust para SoupDB: conecta clientes al pool del gestor vía TCP/IP.

pub mod connection_manager;
pub mod protocol;
pub mod scenarios;
pub mod tcp_server;
pub mod transaction_manager;
pub mod transport;

pub use connection_manager::{Connection, DriverError, QueryResult};
pub use protocol::{Column, Frame, ResultSet, Value};
pub use transaction_manager::{Transaction, TransactionManager};
pub use transport::{TcpTransport, Transport};

#[cfg(feature = "tls")]
pub use transport::{default_tls_config, TlsTransport};
