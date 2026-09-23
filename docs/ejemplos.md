# Ejemplos de uso

## Autocommit

```rust
use rsoup::transaction_manager::TransactionManager;
use rsoup::{QueryResult, Value};

let mut manager = TransactionManager::connect("127.0.0.1", 55432).await?;
manager.ping().await?;

match manager.query("SELECT * FROM accounts").await? {
    QueryResult::ResultSet(rs) => {
        for row in &rs.rows {
            let cells: Vec<String> = row
                .iter()
                .map(|v| match v {
                    Value::Null => "NULL".to_string(),
                    Value::Int(i) => i.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::Text(t) => t.clone(),
                    Value::Bool(b) => b.to_string(),
                })
                .collect();
            println!("{}", cells.join(", "));
        }
    }
    QueryResult::Affected(n) => println!("{n} filas afectadas"),
}
```

## Transacción atómica

```rust
use rsoup::transaction_manager::TransactionManager;

let mut manager = TransactionManager::connect("127.0.0.1", 55432).await?;

manager.begin().await?;
manager.query("INSERT INTO accounts VALUES (98, 8888)").await?;
manager.commit().await?; // o manager.rollback().await? para deshacer
```

## Integración con Tauri (frontend SoupChef)

El backend Rust de la app de escritorio (carpeta `src-tauri`) registra comandos que usan el driver. El frontend Svelte los invoca con `invoke`.

**Comando Tauri (Rust, `src-tauri/src/lib.rs`):**

```rust
use rsoup::transaction_manager::TransactionManager;
use rsoup::QueryResult;

#[tauri::command]
async fn query_sql(
    host: String,
    port: u16,
    sql: String,
) -> Result<QueryResult, String> {
    let mut manager = TransactionManager::connect(&host, port).await
        .map_err(|e| e.to_string())?;
    manager.query(&sql).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn begin_transaction(host: String, port: u16) -> Result<(), String> {
    let mut manager = TransactionManager::connect(&host, port).await
        .map_err(|e| e.to_string())?;
    manager.begin().await.map_err(|e| e.to_string())
}
```

> Nota: para mantener una transacción viva entre comandos, el `TransactionManager` debe vivir en estado de la app (p. ej. en un `tauri::State`), no crearse por comando.

**Invocación desde Svelte (`SoupChef/src/lib/...`):**

```ts
import { invoke } from "@tauri-apps/api/core";

interface QueryResult {
  columns: { name: string; type_code: number; length: number }[];
  rows: unknown[][];
}

async function runSql(sql: string, host = "127.0.0.1", port = 55432) {
  return await invoke<QueryResult>("query_sql", { host, port, sql });
}

const rows = await runSql("SELECT * FROM accounts");
console.log(rows.columns, rows.rows);
```

Con **specta** las estructuras de `ResultSet`/`QueryResult` se exportan como tipos TypeScript al frontend (errores de compilación automáticos si el contrato cambia), según la arquitectura del proyecto.