# API pública del driver

Todos los tipos se re-exportan desde la raíz (`rsoup::*`).

## Tipos

| Tipo | Descripción |
|---|---|
| `Connection` | Conexión de bajo nivel hacia el pool del gestor |
| `TransactionManager` | API de alto nivel con seguimiento de la transacción activa |
| `Transaction<'a>` | Guard de transacción sobre una conexión |
| `QueryResult` | `ResultSet` (SELECT) o `Affected(u32)` (DML/DDL) |
| `DriverError { code, message }` | Error devuelto por el gestor (frame `ERROR`) |
| `ResultSet { columns, rows }` · `Column` · `Value` | Modelo del resultado decodificado |

`Value` (celda): `Null`, `Int(i32)`, `Float(f64)`, `Text(String)`, `Bool(bool)`.

## `Connection` — conexión de bajo nivel

| Función | Firma | Acción |
|---|---|---|
| `connect` | `async fn(host: &str, port: u16) -> io::Result<Self>` | Abre el socket TCP hacia el pool del gestor |
| `ping` | `async fn(&mut self) -> io::Result<()>` | Handshake PING/PONG |
| `query` | `async fn(&mut self, sql: &str) -> Result<QueryResult, DriverError>` | Ejecuta SQL (autocommit o dentro de la tx activa) |
| `begin` | `async fn(&mut self) -> Result<(), DriverError>` | `BEGIN TRANSACTION` |
| `commit` | `async fn(&mut self) -> Result<(), DriverError>` | `COMMIT` (libera locks) |
| `rollback` | `async fn(&mut self) -> Result<(), DriverError>` | `ROLLBACK` (deshace y libera locks) |
| `active_transactions` | `fn(&self) -> u32` | Transacciones activas reportadas (0 = autocommit) |

## `TransactionManager` — API recomendada

| Función | Firma | Acción |
|---|---|---|
| `connect` | `async fn(host: &str, port: u16) -> anyhow::Result<Self>` | Conecta al gestor |
| `ping` | `async fn(&mut self) -> io::Result<()>` | Handshake |
| `query` | `async fn(&mut self, sql: &str) -> Result<QueryResult, DriverError>` | SQL en autocommit o en la tx activa |
| `begin` | `async fn(&mut self) -> Result<(), DriverError>` | Inicia transacción (trackea estado) |
| `commit` | `async fn(&mut self) -> Result<(), DriverError>` | Confirma la tx activa |
| `rollback` | `async fn(&mut self) -> Result<(), DriverError>` | Deshace la tx activa |
| `in_transaction` | `fn(&self) -> bool` | ¿Hay una transacción activa? |

## `Transaction<'a>` — guard de transacción

| Función | Firma | Acción |
|---|---|---|
| `begin` | `async fn(connection: &'a mut Connection) -> Result<Self, DriverError>` | Toma una conexión en transacción |
| `query` | `async fn(&mut self, sql) -> Result<QueryResult, DriverError>` | SQL dentro de la tx (rechaza si no está activa) |
| `commit` | `async fn(self) -> Result<(), DriverError>` | Confirma (consume `self`) |
| `rollback` | `async fn(self) -> Result<(), DriverError>` | Deshace (consume `self`) |

Ver [ejemplos.md](ejemplos.md) para uso completo.