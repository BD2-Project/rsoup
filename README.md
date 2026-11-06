# rsoup

Driver **Rust** para SoupDB: conecta clientes al pool de conexiones del gestor (TCP/IP clásico, capa 4) usando el protocolo binario **v1.1**.

## Instalación

El crate se publica en crates.io como **`soupdb-driver`**; el nombre de librería se mantiene como `rsoup`, así que se importa igual:

```bash
cargo add soupdb-driver
```

```rust
use rsoup::transaction_manager::TransactionManager;
```

## Documentación

| Página | Contenido |
|---|---|
| [docs/index.md](docs/index.md) | Visión general y arquitectura de comunicación |
| [docs/api.md](docs/api.md) | API pública del driver |
| [docs/protocolo.md](docs/protocolo.md) | Protocolo binario v1.1 |
| [docs/ejemplos.md](docs/ejemplos.md) | Ejemplos de uso, incl. Tauri |
| [docs/versionado.md](docs/versionado.md) | Versionado y compatibilidad con el gestor |

## Módulos

| Módulo | Responsabilidad |
|---|---|
| `src/protocol.rs` | Codec del protocolo binario v1.1 (mismo contrato que `SoupDB/engine/transactions/protocol.py`) |
| `src/transport.rs` | Abstracción de transporte: `TcpTransport` (default) y `TlsTransport` (feature `tls`) |
| `src/tcp_server.rs` | Shim de compatibilidad (`Socket` = `TcpTransport`) |
| `src/connection_manager.rs` | `Connection`: operaciones de alto nivel (query, prepared statements, begin, commit, rollback) |
| `src/transaction_manager.rs` | `TransactionManager` / `Transaction`: API de transacciones del cliente |
| `src/scenarios.rs` | Escenarios preconfigurados (ping, select, commit, rollback, error, spatial, prepared, vector) |
| `src/bin/soup_tui.rs` | Demo TUI (ratatui) automatizable con `--scenario` |
| `src/main.rs` | Binario demo que conecta al gestor y ejecuta SQL/transacciones |

## Capacidades v1.1

- **Tipos por nombre**: `Column` expone `type_name` + `encoding` + `size` + `length`; los tipos desconocidos llegan como `Value::Unknown { type_name, bytes }`.
- **`VECTOR(n)`**: `Value::Vector(Vec<f64>)`.
- **Prepared statements**: `Connection::prepare/execute/deallocate` (opcodes `PREPARE/EXECUTE/DEALLOCATE`).
- **Transporte TLS opt-in**: feature cargo `tls` (`Connection::connect_tls`). El gestor aún no hace handshake TLS; el default sigue siendo TCP plano.

## Demo TUI automatizable (`soup_tui`)

```bash
cargo run --bin soup_tui                # TUI interactiva
cargo run --bin soup_tui -- --list      # lista los escenarios
cargo run --bin soup_tui -- --scenario ping   # headless (exit 0/1)
```

Escenarios: `ping`, `select`, `commit`, `rollback`, `error`, `spatial`, `prepared`, `vector` — verifica conexión, consulta, transacciones (COMMIT persiste / ROLLBACK no persiste), errores, consultas espaciales (decodificación de columnas `POINT` vía k-NN), prepared statements y decodificación de `VECTOR`. Conecta a `DRIVER_HOST`/`DRIVER_PORT` (default `127.0.0.1:55432`).

> `prepared` y `vector` requieren que el gestor exponga el contrato v1.1 (prepared statements y tipo `VECTOR`). En el E2E están detrás de la variable de repositorio `RSOUP_E2E_V11=true`.

## Uso desde Tauri (frontend SoupChef)

El backend Rust de la app (`src-tauri`) registra comandos que usan el driver; Svelte los invoca con `invoke`.

```rust
// src-tauri/src/lib.rs
use rsoup::transaction_manager::TransactionManager;
use rsoup::QueryResult;

#[tauri::command]
async fn query_sql(host: String, port: u16, sql: String) -> Result<QueryResult, String> {
    let mut manager = TransactionManager::connect(&host, port).await
        .map_err(|e| e.to_string())?;
    manager.query(&sql).await.map_err(|e| e.to_string())
}
```

```ts
// SoupChef/src/lib/db.ts
import { invoke } from "@tauri-apps/api/core";

interface Column {
  name: string;
  type_name: string; // "INT", "VECTOR", ...
  encoding: number;  // 0=native, 1=fixed, 2=variable, 3=opaque
  size: number;
  length: number;
}

interface QueryResult {
  columns: Column[];
  rows: unknown[][];
}

export async function runSql(sql: string, host = "127.0.0.1", port = 55432) {
  return await invoke<QueryResult>("query_sql", { host, port, sql });
}
```

## Uso directo (demo)

```bash
# Requiere el gestor corriendo (SoupDB):
uv run python scripts/run_server.py

# Conecta el driver (default 127.0.0.1:55432; env DRIVER_HOST/DRIVER_PORT):
cargo run
```

## Tests

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check --features tls        # verifica el transporte TLS opt-in
```

- **Unit** (`src/*`): codec del protocolo v1.1 (incl. `VECTOR`, opaco, legacy v1), transporte TCP y payloads de prepared statements.
- **Integración self-contained** (`tests/mock_gestor.rs`): el driver contra un gestor simulado (sockets reales, protocolo v1.1, prepared statements).
- **E2E punta a punta** (`.github/workflows/e2e.yml`): arranca el gestor real (SoupDB) y ejecuta los escenarios; los de v1.1 se activan con `RSOUP_E2E_V11=true`.

> Los tests intensivos están **activos por defecto**. Para desactivarlos:
> - `RSOUP_SKIP_INTEGRATION=1 cargo test` → salta la integración con mock.
> - Desactivar el workflow E2E: variable de repo `RSOUP_E2E_DISABLED=true`, o en ejecución manual `run_e2e=false`.

Protocolo v1.1 documentado en [docs/protocolo.md](docs/protocolo.md); la especificación canónica vive en el repo del gestor.
