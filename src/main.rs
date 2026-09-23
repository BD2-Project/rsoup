//! Binario demo del driver rsoup.
//!
//! Conecta al gestor (env DRIVER_HOST / DRIVER_PORT, default 127.0.0.1:55432),
//! hace ping, ejecuta una consulta y demuestra una transacción
//! (BEGIN/INSERT/ROLLBACK) sobre el protocolo v1.

use anyhow::{Context, Result};
use rsoup::connection_manager::QueryResult;
use rsoup::transaction_manager::TransactionManager;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 55432;

fn host_port() -> (String, u16) {
    let host = std::env::var("DRIVER_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = std::env::var("DRIVER_PORT")
        .map(|p| p.parse().unwrap_or(DEFAULT_PORT))
        .unwrap_or(DEFAULT_PORT);
    (host, port)
}

fn print_result(result: &QueryResult) {
    match result {
        QueryResult::ResultSet(rs) => {
            let headers: Vec<String> = rs.columns.iter().map(|c| c.name.clone()).collect();
            println!("    columnas: {}", headers.join(", "));
            for row in &rs.rows {
                let cells: Vec<String> = row
                    .iter()
                    .map(|v| match v {
                        rsoup::Value::Null => "NULL".to_string(),
                        rsoup::Value::Int(i) => i.to_string(),
                        rsoup::Value::Float(f) => f.to_string(),
                        rsoup::Value::Text(t) => t.clone(),
                        rsoup::Value::Bool(b) => b.to_string(),
                    })
                    .collect();
                println!("    fila: {}", cells.join(", "));
            }
        }
        QueryResult::Affected(n) => println!("    filas afectadas: {n}"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let (host, port) = host_port();
    println!("rsoup demo -> conectando a {host}:{port}");

    let mut manager = TransactionManager::connect(&host, port)
        .await
        .context("no se pudo conectar al gestor")?;

    // ping
    manager.ping().await.context("fallo el ping al gestor")?;
    println!("    ping ok");

    // consulta simple (autocommit)
    println!("consulta: SELECT * FROM accounts");
    let result = manager
        .query("SELECT * FROM accounts")
        .await
        .context("fallo la consulta")?;
    print_result(&result);

    // transacción con rollback
    println!("transaccion: BEGIN -> INSERT -> ROLLBACK");
    manager.begin().await?;
    manager
        .query("INSERT INTO accounts VALUES (99, 9999)")
        .await?;
    manager.rollback().await?;
    println!("    rollback ok");

    // transacción con commit
    println!("transaccion: BEGIN -> INSERT -> COMMIT");
    manager.begin().await?;
    manager
        .query("INSERT INTO accounts VALUES (98, 8888)")
        .await?;
    manager.commit().await?;
    println!("    commit ok");

    println!("consulta final: SELECT * FROM accounts");
    let result = manager
        .query("SELECT * FROM accounts")
        .await
        .context("fallo la consulta final")?;
    print_result(&result);

    Ok(())
}
