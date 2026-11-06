# Protocolo binario v1.1

Contrato entre el driver rsoup (cliente) y el gestor SoupDB (servidor). El formato canónico se documenta en el repo del gestor:

- **Especificación completa:** https://github.com/BD2-Project/SoupDB/blob/dev/docs/protocolo.md
- **Codec Python (servidor):** `SoupDB/engine/transactions/protocol.py`
- **Codec Rust (driver):** `rsoup/src/protocol.rs` (este repo)

> Si el codec Rust y el Python difieren, **manda la especificación** en `docs/protocolo.md` del gestor. La implementación del driver se reconcilia contra ella.

## Frame

```
┌────────┬─────────┬─────────┬─────────────┬────────────────┐
│ magic  │ version │ opcode  │ length      │ payload        │
│ 2B "SP"│ 1B 0x01 │ 1B      │ 4B (u32 BE) │ (length bytes) │
└────────┴─────────┴─────────┴─────────────┴────────────────┘
```

- Requests: `PING=0x01` · `BEGIN=0x03` · `COMMIT=0x04` · `ROLLBACK=0x05` · `QUERY=0x06` · `PREPARE=0x07` · `EXECUTE=0x08` · `DEALLOCATE=0x09`
- Responses: `PONG=0x02` · `OK=0x10` · `RESULT=0x11` · `ERROR=0x12`

## Tipos y encoding

Los 6 tipos nativos conservan sus códigos y tags originales:

| Tipo | Código v1 | Tag de valor |
|---|---|---|
| `INT` | `0x01` | `INT=0x01` |
| `FLOAT` | `0x02` | `FLOAT=0x02` |
| `VARCHAR` | `0x03` | `TEXT=0x03` |
| `TEXT` | `0x04` | `TEXT=0x03` |
| `BOOL` | `0x05` | `BOOL=0x04` |
| `POINT` | `0x06` | `POINT=0x05` (dos `f64` BE: x=longitud, y=latitud) |

Tags nuevos de v1.1:

- `VECTOR=0x06`: `dimension u16 BE` + `dimension × f64 BE` (tipo `VECTOR(n)`).
- `OPAQUE=0x07`: `length u32 BE` + bytes (tipo extensible desconocido para el cliente).

## Header de columna en `RESULT` (v1.1)

```
name:      u16 BE len + UTF-8
type_name: u16 BE len + UTF-8      ("INT", "FLOAT", "VARCHAR", "TEXT", "BOOL", "POINT", "VECTOR", o tipo extensible)
encoding:  u8                      (0=native, 1=fixed, 2=variable, 3=opaque)
size:      u32 BE                  (bytes para tipos fixed; 0 en el resto)
length:    u16 BE                  (longitud declarada, p. ej. VARCHAR(n); 0 en el resto)
```

- `encoding=opaque` indica que el driver no conoce el tipo: el valor llega como `OPAQUE` y se expone como `Value::Unknown { type_name, bytes }`.
- El decoder **v1 legado** (`decode_resultset_v1`) sigue disponible para gestores previos a v1.1; se retirará cuando el E2E apunte al gestor v1.1 definitivo.

## Prepared statements

- **`PREPARE`**: `name u16-len + UTF-8` + `sql u32-len + UTF-8` → `OK` / `ERROR`.
- **`EXECUTE`**: `name u16-len + UTF-8` + `param_count u16` + parámetros (cada uno `TAG` + valor) → `RESULT` / `OK` / `ERROR`.
- **`DEALLOCATE`**: `name u16-len + UTF-8` → `OK` / `ERROR`.

Los parámetros usan los mismos tags que las celdas (`NULL/INT/FLOAT/TEXT/BOOL/POINT/VECTOR/OPAQUE`). El caché de planes y su invalidación por DDL viven en el gestor.

## Códigos de error

| Código | Nombre |
|---|---|
| `0x00` | `ERR_GENERIC` |
| `0x01` | `ERR_PARSE` |
| `0x02` | `ERR_EXECUTION` |
| `0x03` | `ERR_TRANSACTION` |
| `0x04` | `ERR_LOCK_TIMEOUT` |
| `0x05` | `ERR_DEADLOCK` |
| `0x06` | `ERR_RECORD_NOT_FOUND` |
| `0x07` | `ERR_CONTRACT` |
| `0x08` | `ERR_UNSUPPORTED` |

## Transporte

El driver usa **TCP plano** por defecto. El transporte **TLS es opt-in** (feature cargo `tls`, `Connection::connect_tls`) y todavía no puede completar un handshake real porque el gestor no lo implementa (trabajo futuro del gestor). Ver [index.md](index.md).
