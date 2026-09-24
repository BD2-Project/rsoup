//! Escenarios preconfigurados del driver (automatizables y para la TUI).
//!
//! Cada escenario conecta al gestor y verifica una funcionalidad del driver.
//! En modo headless (`--scenario`) se imprimen y se traducen a exit code 0/1;
//! la TUI los ejecuta interactivamente.

use crate::connection_manager::{DriverError, QueryResult};
use crate::transaction_manager::TransactionManager;
use crate::Value;

/// Resultado de un escenario.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    pub name: &'static str,
    pub ok: bool,
    pub summary: String,
}

/// Escenarios disponibles.
pub const SCENARIOS: &[&str] = &["ping", "select", "commit", "rollback", "error"];

fn driver_err(error: impl std::fmt::Display) -> DriverError {
    DriverError {
        code: crate::protocol::ERR_GENERIC,
        message: error.to_string(),
    }
}

/// Ejecuta un escenario por nombre.
pub async fn run(name: &str, host: &str, port: u16) -> Result<ScenarioResult, DriverError> {
    match name {
        "ping" => scenario_ping(host, port).await,
        "select" => scenario_select(host, port).await,
        "commit" => scenario_commit(host, port).await,
        "rollback" => scenario_rollback(host, port).await,
        "error" => scenario_error(host, port).await,
        other => Err(DriverError {
            code: crate::protocol::ERR_GENERIC,
            message: format!("escenario desconocido: {other}"),
        }),
    }
}

async fn scenario_ping(host: &str, port: u16) -> Result<ScenarioResult, DriverError> {
    let mut manager = TransactionManager::connect(host, port)
        .await
        .map_err(driver_err)?;
    manager.ping().await.map_err(driver_err)?;
    Ok(ScenarioResult {
        name: "ping",
        ok: true,
        summary: "handshake PING/PONG correcto".to_string(),
    })
}

async fn scenario_select(host: &str, port: u16) -> Result<ScenarioResult, DriverError> {
    let mut manager = TransactionManager::connect(host, port)
        .await
        .map_err(driver_err)?;
    let result = manager
        .query("SELECT id, balance FROM accounts ORDER BY id")
        .await?;
    match result {
        QueryResult::ResultSet(rs) => {
            let expected: Vec<(i32, i32)> = vec![(1, 100), (2, 200)];
            let mut rows: Vec<(i32, i32)> = Vec::new();
            for row in &rs.rows {
                if let (Value::Int(id), Value::Int(balance)) = (&row[0], &row[1]) {
                    rows.push((*id, *balance));
                }
            }
            let ok = rows == expected;
            Ok(ScenarioResult {
                name: "select",
                ok,
                summary: format!(
                    "{} filas | esperadas {:?} | obtenidas {:?}",
                    rows.len(),
                    expected,
                    rows
                ),
            })
        }
        QueryResult::Affected(_) => Ok(ScenarioResult {
            name: "select",
            ok: false,
            summary: "SELECT devolvio OK, se esperaba ResultSet".to_string(),
        }),
    }
}

async fn scenario_commit(host: &str, port: u16) -> Result<ScenarioResult, DriverError> {
    let mut manager = TransactionManager::connect(host, port)
        .await
        .map_err(driver_err)?;
    manager.begin().await?;
    manager
        .query("INSERT INTO accounts VALUES (90, 1000)")
        .await?;
    manager.commit().await?;
    let present = row_exists(&mut manager, 90).await?;
    Ok(ScenarioResult {
        name: "commit",
        ok: present,
        summary: if present {
            "BEGIN+INSERT+COMMIT persistio la fila 90".to_string()
        } else {
            "COMMIT no persistio la fila 90".to_string()
        },
    })
}

async fn scenario_rollback(host: &str, port: u16) -> Result<ScenarioResult, DriverError> {
    let mut manager = TransactionManager::connect(host, port)
        .await
        .map_err(driver_err)?;
    manager.begin().await?;
    manager
        .query("INSERT INTO accounts VALUES (91, 2000)")
        .await?;
    manager.rollback().await?;
    let present = row_exists(&mut manager, 91).await?;
    Ok(ScenarioResult {
        name: "rollback",
        ok: !present,
        summary: if present {
            "ROLLBACK no deshizo la fila 91".to_string()
        } else {
            "BEGIN+INSERT+ROLLBACK deshizo la fila 91".to_string()
        },
    })
}

async fn scenario_error(host: &str, port: u16) -> Result<ScenarioResult, DriverError> {
    let mut manager = TransactionManager::connect(host, port)
        .await
        .map_err(driver_err)?;
    match manager.query("BOGUS SQL").await {
        Err(error) if error.code == crate::protocol::ERR_PARSE => Ok(ScenarioResult {
            name: "error",
            ok: true,
            summary: format!("SQL invalido -> ERR_PARSE: {}", error.message),
        }),
        Err(error) => Ok(ScenarioResult {
            name: "error",
            ok: false,
            summary: format!("codigo de error inesperado {}", error.code),
        }),
        Ok(_) => Ok(ScenarioResult {
            name: "error",
            ok: false,
            summary: "no se recibio ningun error".to_string(),
        }),
    }
}

async fn row_exists(manager: &mut TransactionManager, id: i32) -> Result<bool, DriverError> {
    match manager.query("SELECT id FROM accounts").await? {
        QueryResult::ResultSet(rs) => Ok(rs
            .rows
            .iter()
            .any(|row| matches!(row.first(), Some(Value::Int(value)) if *value == id))),
        QueryResult::Affected(_) => Ok(false),
    }
}
