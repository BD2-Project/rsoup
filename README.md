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
| `src/main.rs` | Binario demo que conecta al gestor y ejecuta SQL/transacciones |

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

Protocolo v1 documentado en `docs/protocolo.md` (repo del gestor).