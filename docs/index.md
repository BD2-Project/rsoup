# rsoup — Driver del gestor SoupDB

**rsoup** es el driver de red del proyecto SoupDB, escrito en **Rust**. Es el **cliente** que los consumidores (frontend SoupChef vía Tauri, aplicaciones, scripts) usan para conectarse al **pool de conexiones del gestor** mediante **TCP/IP clásico** (capa 4).

```
+---------------------+      TCP/IP (protocolo binario v1.1)    +--------------------------+
| Cliente (Svelte/     | <------------------------------------> | SoupDB gestor (Python)   |
| SoupChef + Tauri)    |   QUERY / BEGIN / COMMIT / ROLLBACK     | ConnectionHandler (pool) |
|        |             |   PREPARE / EXECUTE / DEALLOCATE        | DRIVER_PORT 55432        |
|        | invoke      |   + RESULT / OK / ERROR                 |                          |
|        v             |                                        | MAX_CONNECTIONS 100      |
|   rsoup (Rust)       |                                        +--------------------------+
|   driver cliente     |
+---------------------+
```

El pool de conexiones lo gestiona el gestor (`engine/transactions/connection_handler.py`); rsoup solo abre una conexión a ese puerto y habla el protocolo v1.1.

## Contenido de esta documentación

| Página | Contenido |
|---|---|
| [api.md](api.md) | API pública del driver (Connection, TransactionManager, Transaction, tipos) |
| [protocolo.md](protocolo.md) | Protocolo binario v1.1 (contrato con el gestor) |
| [ejemplos.md](ejemplos.md) | Ejemplos de uso, incluyendo integración con Tauri (frontend) |
| [versionado.md](versionado.md) | Convención de versiones y matriz de compatibilidad con el gestor |

## Configuración

| Variable | Default | Uso |
|---|---|---|
| `DRIVER_HOST` | `127.0.0.1` | Host del gestor |
| `DRIVER_PORT` | `55432` | Puerto del pool del gestor |

## Capacidades v1.1

- **Tipos por nombre** (`Column::type_name`) + `encoding`; tipos desconocidos llegan como `Value::Unknown` (bytes opacos) sin romper la conexión.
- **`VECTOR(n)`** de primera clase (`Value::Vector`).
- **Prepared statements** (`prepare` / `execute` / `deallocate`) con parámetros tipados.
- **Transporte TLS opt-in** (feature cargo `tls`). El gestor todavía no implementa el handshake TLS, así que la conexión por defecto sigue siendo TCP plano; la abstracción queda lista para cuando el servidor lo soporte.

## Requisitos

- El gestor SoupDB corriendo: `uv run python scripts/run_server.py` (repo SoupDB).
- Rust (toolchain estable) + tokio.
