//! Gestión de la conexión del driver hacia el gestor.
//!
//! `Connection` envuelve el socket TCP y expone operaciones de alto nivel del
//! protocolo v1. La conexión apunta al puerto del pool del gestor; el pool en
//! sí lo gestiona el gestor (ver `ConnectionHandler` en SoupDB).

use std::io;

use crate::protocol::{
    decode_error, decode_ok, decode_resultset, encode_query, Frame, ResultSet, OP_BEGIN, OP_COMMIT,
    OP_ERROR, OP_OK, OP_PING, OP_PONG, OP_QUERY, OP_RESULT, OP_ROLLBACK,
};
use crate::tcp_server::Socket;

/// Error devuelto por el gestor en un frame ERROR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverError {
    pub code: u8,
    pub message: String,
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "driver error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for DriverError {}

impl From<io::Error> for DriverError {
    fn from(err: io::Error) -> Self {
        DriverError {
            code: crate::protocol::ERR_GENERIC,
            message: err.to_string(),
        }
    }
}

impl From<crate::protocol::ProtocolError> for DriverError {
    fn from(err: crate::protocol::ProtocolError) -> Self {
        DriverError {
            code: crate::protocol::ERR_GENERIC,
            message: err.to_string(),
        }
    }
}

/// Resultado de una consulta: RESULT con columnas/filas, u OK con afectadas.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryResult {
    ResultSet(ResultSet),
    Affected(u32),
}

/// Conexión del driver hacia el gestor.
pub struct Connection {
    socket: Socket,
    /// Número de transacciones activas reportadas por el gestor.
    active_transactions: u32,
}

impl Connection {
    /// Conecta al gestor (host, puerto del pool).
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let socket = Socket::connect(host, port).await?;
        Ok(Self {
            socket,
            active_transactions: 0,
        })
    }

    /// Ping/pong de handshake.
    pub async fn ping(&mut self) -> io::Result<()> {
        let frame = Frame {
            opcode: OP_PING,
            payload: Vec::new(),
        };
        self.socket.send_frame(&frame).await?;
        let response = self.socket.recv_frame().await?;
        if response.opcode != OP_PONG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected PONG, got opcode {}", response.opcode),
            ));
        }
        Ok(())
    }

    /// Ejecuta una sentencia SQL (autocommit o dentro de la transacción activa).
    pub async fn query(&mut self, sql: &str) -> Result<QueryResult, DriverError> {
        let frame = Frame {
            opcode: OP_QUERY,
            payload: encode_query(sql),
        };
        self.socket.send_frame(&frame).await?;
        self.read_query_result().await
    }

    /// Inicia una transacción (BEGIN TRANSACTION).
    pub async fn begin(&mut self) -> Result<(), DriverError> {
        self.tx_command(OP_BEGIN).await
    }

    /// Confirma la transacción (COMMIT).
    pub async fn commit(&mut self) -> Result<(), DriverError> {
        self.tx_command(OP_COMMIT).await
    }

    /// Deshace la transacción (ROLLBACK).
    pub async fn rollback(&mut self) -> Result<(), DriverError> {
        self.tx_command(OP_ROLLBACK).await
    }

    /// Número de transacciones activas en esta conexión (0 = autocommit).
    pub fn active_transactions(&self) -> u32 {
        self.active_transactions
    }

    async fn tx_command(&mut self, opcode: u8) -> Result<(), DriverError> {
        let frame = Frame {
            opcode,
            payload: Vec::new(),
        };
        self.socket.send_frame(&frame).await?;
        let response = self.socket.recv_frame().await?;
        match response.opcode {
            OP_OK => {
                self.active_transactions = match opcode {
                    OP_BEGIN => self.active_transactions + 1,
                    OP_COMMIT | OP_ROLLBACK => self.active_transactions.saturating_sub(1),
                    _ => self.active_transactions,
                };
                Ok(())
            }
            OP_ERROR => {
                let (code, message) = decode_error(&response.payload)?;
                Err(DriverError { code, message })
            }
            other => Err(DriverError {
                code: crate::protocol::ERR_GENERIC,
                message: format!("unexpected opcode {other} for transaction command"),
            }),
        }
    }

    async fn read_query_result(&mut self) -> Result<QueryResult, DriverError> {
        let response = self.socket.recv_frame().await?;
        match response.opcode {
            OP_RESULT => {
                let rs = decode_resultset(&response.payload)?;
                Ok(QueryResult::ResultSet(rs))
            }
            OP_OK => {
                let affected = decode_ok(&response.payload)?;
                Ok(QueryResult::Affected(affected))
            }
            OP_ERROR => {
                let (code, message) = decode_error(&response.payload)?;
                Err(DriverError { code, message })
            }
            other => Err(DriverError {
                code: crate::protocol::ERR_GENERIC,
                message: format!("unexpected opcode {other} for query"),
            }),
        }
    }
}
