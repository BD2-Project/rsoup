# rsoup

Driver **Rust** para SoupDB: conecta clientes al pool de conexiones del gestor (TCP/IP clásico, capa 4) usando el protocolo binario v1.

## Documentación

| Página | Contenido |
|---|---|
| [docs/index.md](docs/index.md) | Visión general y arquitectura de comunicación |
| [docs/api.md](docs/api.md) | API pública del driver |
| [docs/protocolo.md](docs/protocolo.md) | Protocolo binario v1 |
| [docs/ejemplos.md](docs/ejemplos.md) | Ejemplos de uso, incl. Tauri |

## Módulos

| Módulo | Responsabilidad |
|---|---|
| `src/protocol.rs` | Codec del protocolo binario v1 (mismo contrato que `SoupDB/engine/transactions/protocol.py`) |
| `src/tcp_server.rs` | Gestión de sockets TCP (tokio): connect, read_frame / write_frame |
| `src/connection_manager.rs` | `Connection`: operaciones de alto nivel (query, begin, commit, rollback) |
| `src/transaction_manager.rs` | `TransactionManager` / `Transaction`: API de transacciones del cliente |
| `src/scenarios.rs` | Escenarios preconfigurados (ping, select, commit, rollback, error) |
| `src/bin/soup_tui.rs` | Demo TUI (ratatui) automatizable con `--scenario` |
| `src/main.rs` | Binario demo que conecta al gestor y ejecuta SQL/transacciones |

## Demo TUI automatizable (`soup_tui`)

```bash
cargo run --bin soup_tui                # TUI interactiva
cargo run --bin soup_tui -- --list      # lista los escenarios
cargo run --bin soup_tui -- --scenario ping   # headless (exit 0/1)
```

Escenarios: `ping`, `select`, `commit`, `rollback`, `error` — verifica conexión, consulta, transacciones (COMMIT persiste / ROLLBACK no persiste) y errores. Conecta a `DRIVER_HOST`/`DRIVER_PORT` (default `127.0.0.1:55432`).

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

interface QueryResult {
  columns: { name: string; type_code: number; length: number }[];
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
```

- **Unit** (`src/*`): codec del protocolo y helpers.
- **Integración self-contained** (`tests/mock_gestor.rs`): el driver contra un gestor simulado (sockets reales, protocolo v1).
- **E2E punta a punta** (`.github/workflows/e2e.yml`): arranca el gestor real (SoupDB) y ejecuta los 5 escenarios.

> Los tests intensivos están **activos por defecto**. Para desactivarlos:
> - `RSOUP_SKIP_INTEGRATION=1 cargo test` → salta la integración con mock.
> - Desactivar el workflow E2E: variable de repo `RSOUP_E2E_DISABLED=true`, o en ejecución manual `run_e2e=false`.

Protocolo v1 documentado en `docs/protocolo.md` (repo del gestor).