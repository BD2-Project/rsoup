# Protocolo binario v1

Contrato entre el driver rsoup (cliente) y el gestor SoupDB (servidor). El formato canónico se documenta en el repo del gestor:

- **Especificación completa:** https://github.com/BD2-Project/SoupDB/blob/dev/docs/protocolo.md
- **Codec Python (servidor):** `SoupDB/engine/transactions/protocol.py`
- **Codec Rust (driver):** `rsoup/src/protocol.rs` (este repo)

## Resumen

```
┌────────┬─────────┬─────────┬─────────────┬────────────────┐
│ magic  │ version │ opcode  │ length      │ payload        │
│ 2B "SP"│ 1B 0x01 │ 1B      │ 4B (u32 BE) │ (length bytes) │
└────────┴─────────┴─────────┴─────────────┴────────────────┘
```

- Requests: `PING=0x01` · `BEGIN=0x03` · `COMMIT=0x04` · `ROLLBACK=0x05` · `QUERY=0x06`
- Responses: `PONG=0x02` · `OK=0x10` · `RESULT=0x11` · `ERROR=0x12`
- El payload de `RESULT` serializa columnas y filas con tags `NULL/INT/FLOAT/TEXT/BOOL`.

Los dos codecs deben implementar exactamente el mismo formato; si difieren, **manda la especificación** en `docs/protocolo.md` del gestor.