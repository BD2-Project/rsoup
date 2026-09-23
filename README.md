# rsoup

Driver **Rust** para SoupDB: conecta clientes al pool de conexiones del gestor (TCP/IP clásico, capa 4) usando el protocolo binario v1.

## Módulos

| Módulo | Responsabilidad |
|---|---|
| `src/protocol.rs` | Codec del protocolo binario v1 (mismo contrato que `SoupDB/engine/transactions/protocol.py`) |
| `src/tcp_server.rs` | Gestión de sockets TCP (tokio): connect, read_frame / write_frame |
| `src/connection_manager.rs` | `Connection`: operaciones de alto nivel (query, begin, commit, rollback) |
| `src/transaction_manager.rs` | `TransactionManager` / `Transaction`: API de transacciones del cliente |
| `src/main.rs` | Binario demo que conecta al gestor y ejecuta SQL/transacciones |

## Uso (demo)

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

Protocolo v1 documentado en `docs/protocolo.md` del repo SoupDB.