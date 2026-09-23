# rsoup — Driver del gestor SoupDB

**rsoup** es el driver de red del proyecto SoupDB, escrito en **Rust**. Es el **cliente** que los consumidores (frontend SoupChef vía Tauri, aplicaciones, scripts) usan para conectarse al **pool de conexiones del gestor** mediante **TCP/IP clásico** (capa 4).

```
+---------------------+      TCP/IP (protocolo binario v1)      +--------------------------+
| Cliente (Svelte/     | <------------------------------------> | SoupDB gestor (Python)   |
| SoupChef + Tauri)    |   QUERY / BEGIN / COMMIT / ROLLBACK     | ConnectionHandler (pool) |
|        |             |   + RESULT / OK / ERROR                 | DRIVER_PORT 55432        |
|        | invoke      |                                        |                          |
|        v             |                                        | MAX_CONNECTIONS 100      |
|   rsoup (Rust)       |                                        +--------------------------+
|   driver cliente     |
+---------------------+
```

El pool de conexiones lo gestiona el gestor (`engine/transactions/connection_handler.py`); rsoup solo abre una conexión a ese puerto y habla el protocolo v1.

## Contenido de esta documentación

| Página | Contenido |
|---|---|
| [api.md](api.md) | API pública del driver (Connection, TransactionManager, Transaction, tipos) |
| [protocolo.md](protocolo.md) | Protocolo binario v1 (contrato con el gestor) |
| [ejemplos.md](ejemplos.md) | Ejemplos de uso, incluyendo integración con Tauri (frontend) |

## Configuración

| Variable | Default | Uso |
|---|---|---|
| `DRIVER_HOST` | `127.0.0.1` | Host del gestor |
| `DRIVER_PORT` | `55432` | Puerto del pool del gestor |

## Requisitos

- El gestor SoupDB corriendo: `uv run python scripts/run_server.py` (repo SoupDB).
- Rust (toolchain estable) + tokio.