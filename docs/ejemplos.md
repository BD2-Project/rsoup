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
                    Value::Point(x, y) => format!("{y}, {x}"),
                    Value::Vector(values) => format!("{values:?}"),
                    Value::Unknown { type_name, bytes } => {
                        format!("<{type_name}: {} bytes>", bytes.len())
                    }
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

## Prepared statements

```rust
use rsoup::transaction_manager::TransactionManager;
use rsoup::Value;

let mut manager = TransactionManager::connect("127.0.0.1", 55432).await?;

manager
    .prepare("buscar_cuenta", "SELECT id, balance FROM accounts WHERE id = $1")
    .await?;
let result = manager.execute("buscar_cuenta", &[Value::Int(1)]).await?;
manager.deallocate("buscar_cuenta").await?;
```

Los parámetros van **tipados** con `Value` (`Int`, `Float`, `Text`, `Bool`, `Point`, `Vector`, `Unknown`, `Null`).

## Búsqueda vectorial (VECTOR)

```rust
use rsoup::Value;

// El gestor responde una columna `VECTOR(n)` (encoding=fixed).
if let QueryResult::ResultSet(rs) = manager
    .query("SELECT id, embedding FROM imagenes LIMIT 1")
    .await?
{
    assert!(rs.columns[1].is_vector());
    if let Value::Vector(values) = &rs.rows[0][1] {
        println!("dimension = {}", values.len());
    }
}
```

## Tipos extensibles (fallback opaco)

Si el gestor devuelve un tipo que el driver no conoce (`encoding=opaque`), llega como bytes + nombre, sin romper la conexión:

```rust
match &rs.rows[0][1] {
    Value::Unknown { type_name, bytes } => {
        println!("tipo {type_name} ({} bytes)", bytes.len());
    }
    _ => {}
}
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
async fn prepare_statement(
    host: String,
    port: u16,
    name: String,
    sql: String,
) -> Result<(), String> {
    let mut manager = TransactionManager::connect(&host, port).await
        .map_err(|e| e.to_string())?;
    manager.prepare(&name, &sql).await.map_err(|e| e.to_string())
}
```

> Nota: para mantener una transacción viva entre comandos, el `TransactionManager` debe vivir en estado de la app (p. ej. en un `tauri::State`), no crearse por comando.

**Invocación desde Svelte (`SoupChef/src/lib/...`):**

```ts
import { invoke } from "@tauri-apps/api/core";

// v1.1: `type_code` desaparece; la columna trae `type_name` + `encoding`.
interface Column {
  name: string;
  type_name: string;   // "INT", "VECTOR", "MI_TIPO", ...
  encoding: number;    // 0=native, 1=fixed, 2=variable, 3=opaque
  size: number;
  length: number;
}

// Value puede ser null | number | string | boolean | [number, number]
// | number[] (VECTOR) | { type_name: string; bytes: number[] } (opaque).
interface QueryResult {
  columns: Column[];
  rows: unknown[][];
}

async function runSql(sql: string, host = "127.0.0.1", port = 55432) {
  return await invoke<QueryResult>("query_sql", { host, port, sql });
}

const rows = await runSql("SELECT * FROM accounts");
console.log(rows.columns, rows.rows);
```

Con **specta** las estructuras de `ResultSet`/`QueryResult` se exportan como tipos TypeScript al frontend (errores de compilación automáticos si el contrato cambia), según la arquitectura del proyecto.
