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
| `Transport` · `TcpTransport` · `TlsTransport` (feature `tls`) | Abstracción de transporte |

### `Column` (v1.1)

| Campo | Tipo | Uso |
|---|---|---|
| `name` | `String` | Nombre de la columna |
| `type_name` | `String` | `INT`, `FLOAT`, `VARCHAR`, `TEXT`, `BOOL`, `POINT`, `VECTOR` o tipo extensible |
| `encoding` | `u8` | `0=native`, `1=fixed`, `2=variable`, `3=opaque` |
| `size` | `u32` | Bytes para tipos de tamaño fijo (p. ej. `VECTOR(n)` = `n*8`); 0 en el resto |
| `length` | `u16` | Longitud declarada (`VARCHAR(n)`); 0 en el resto |

Helpers: `Column::native(...)`, `is_native()`, `is_opaque()`, `is_vector()`.

> **Migración:** el antiguo campo `type_code: u8` ya no existe. Usa `type_name`.

### `Value` (celda)

`Null`, `Int(i32)`, `Float(f64)`, `Text(String)`, `Bool(bool)`, `Point(f64, f64)` (x = longitud, y = latitud), **`Vector(Vec<f64>)`**, **`Unknown { type_name: String, bytes: Vec<u8> }`** (fallback opaco).

## `Connection` — conexión de bajo nivel

| Función | Firma | Acción |
|---|---|---|
| `connect` | `async fn(host: &str, port: u16) -> io::Result<Self>` | Abre el socket TCP hacia el pool del gestor |
| `connect_tls` | `async fn(host, port, ClientConfig) -> io::Result<Self>` | Abre una conexión TLS (feature `tls`) |
| `with_transport` | `fn(Transport) -> Self` | Construye la conexión sobre un transporte ya abierto |
| `ping` | `async fn(&mut self) -> io::Result<()>` | Handshake PING/PONG |
| `query` | `async fn(&mut self, sql: &str) -> Result<QueryResult, DriverError>` | Ejecuta SQL (autocommit o dentro de la tx activa) |
| `prepare` | `async fn(&mut self, name: &str, sql: &str) -> Result<(), DriverError>` | Registra un prepared statement |
| `execute` | `async fn(&mut self, name: &str, params: &[Value]) -> Result<QueryResult, DriverError>` | Ejecuta un prepared statement con parámetros tipados |
| `deallocate` | `async fn(&mut self, name: &str) -> Result<(), DriverError>` | Libera un prepared statement |
| `begin` / `commit` / `rollback` | `async fn(&mut self) -> Result<(), DriverError>` | Transacciones |
| `active_transactions` | `fn(&self) -> u32` | Transacciones activas reportadas (0 = autocommit) |

## `TransactionManager` — API recomendada

| Función | Firma | Acción |
|---|---|---|
| `connect` | `async fn(host: &str, port: u16) -> anyhow::Result<Self>` | Conecta al gestor |
| `ping` | `async fn(&mut self) -> io::Result<()>` | Handshake |
| `query` | `async fn(&mut self, sql: &str) -> Result<QueryResult, DriverError>` | SQL en autocommit o en la tx activa |
| `prepare` / `execute` / `deallocate` | ver `Connection` | Prepared statements |
| `begin` / `commit` / `rollback` | `async fn(&mut self) -> Result<(), DriverError>` | Transacciones (trackea estado) |
| `in_transaction` | `fn(&self) -> bool` | ¿Hay una transacción activa? |

## `Transaction<'a>` — guard de transacción

| Función | Firma | Acción |
|---|---|---|
| `begin` | `async fn(connection: &'a mut Connection) -> Result<Self, DriverError>` | Toma una conexión en transacción |
| `query` / `prepare` / `execute` / `deallocate` | ver `Connection` | Operaciones dentro de la tx (rechazan si no está activa) |
| `commit` / `rollback` | `async fn(self) -> Result<(), DriverError>` | Confirma / deshace (consume `self`) |

Ver [ejemplos.md](ejemplos.md) para uso completo.
