//! API de transacciones del cliente sobre el protocolo v1.

use crate::connection_manager::{Connection, DriverError, QueryResult};

/// Guard de transacción: toma una conexión y expone query/commit/rollback.
/// Se debe cerrar con `commit` o `rollback` explícito.
pub struct Transaction<'a> {
    connection: &'a mut Connection,
    active: bool,
}

impl<'a> Transaction<'a> {
    /// Inicia una transacción sobre la conexión.
    pub async fn begin(connection: &'a mut Connection) -> Result<Self, DriverError> {
        connection.begin().await?;
        Ok(Self {
            connection,
            active: true,
        })
    }

    /// Ejecuta SQL dentro de la transacción.
    pub async fn query(&mut self, sql: &str) -> Result<QueryResult, DriverError> {
        if !self.active {
            return Err(DriverError {
                code: crate::protocol::ERR_TRANSACTION,
                message: "transaction is not active".to_string(),
            });
        }
        self.connection.query(sql).await
    }

    /// Confirma la transacción.
    pub async fn commit(mut self) -> Result<(), DriverError> {
        self.active = false;
        self.connection.commit().await
    }

    /// Deshace la transacción.
    pub async fn rollback(mut self) -> Result<(), DriverError> {
        self.active = false;
        self.connection.rollback().await
    }
}

/// Gestor de transacciones del lado del cliente: trackea el estado activo
/// sobre una conexión y expone helpers begin/query/commit/rollback.
pub struct TransactionManager {
    connection: Connection,
    active: bool,
}

impl TransactionManager {
    /// Conecta al gestor y prepara el gestor de transacciones.
    pub async fn connect(host: &str, port: u16) -> anyhow::Result<Self> {
        let connection = Connection::connect(host, port).await?;
        Ok(Self {
            connection,
            active: false,
        })
    }

    /// ¿Hay una transacción activa en esta conexión?
    pub fn in_transaction(&self) -> bool {
        self.active
    }

    /// Ping/pong de handshake contra el gestor.
    pub async fn ping(&mut self) -> Result<(), std::io::Error> {
        self.connection.ping().await
    }

    /// Ejecuta SQL en autocommit o dentro de la transacción activa.
    pub async fn query(&mut self, sql: &str) -> Result<QueryResult, DriverError> {
        self.connection.query(sql).await
    }

    /// Inicia una transacción.
    pub async fn begin(&mut self) -> Result<(), DriverError> {
        self.connection.begin().await?;
        self.active = true;
        Ok(())
    }

    /// Confirma la transacción activa.
    pub async fn commit(&mut self) -> Result<(), DriverError> {
        self.connection.commit().await?;
        self.active = false;
        Ok(())
    }

    /// Deshace la transacción activa.
    pub async fn rollback(&mut self) -> Result<(), DriverError> {
        self.connection.rollback().await?;
        self.active = false;
        Ok(())
    }
}
