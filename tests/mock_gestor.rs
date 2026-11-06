//! Integración del driver contra un gestor simulado (protocolo v1).
//!
//! Self-contained: un servidor tokio implementa el protocolo binario v1 y el
//! driver se conecta a él por un socket real. Para saltar estos tests
//! (intensivos de red) usar: `RSOUP_SKIP_INTEGRATION=1 cargo test`.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;

use rsoup::connection_manager::{Connection, DriverError, QueryResult};
use rsoup::protocol::{
    decode_deallocate, decode_execute, decode_prepare, decode_resultset, encode_error,
    encode_resultset, Column, Frame, ResultSet, Value, OP_BEGIN, OP_COMMIT, OP_DEALLOCATE,
    OP_ERROR, OP_EXECUTE, OP_OK, OP_PING, OP_PONG, OP_PREPARE, OP_QUERY, OP_RESULT, OP_ROLLBACK,
    TYPE_NAME_BOOL, TYPE_NAME_FLOAT, TYPE_NAME_INT, TYPE_NAME_POINT, TYPE_NAME_TEXT,
    TYPE_NAME_VECTOR,
};
use rsoup::transaction_manager::{Transaction, TransactionManager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn skip_if_disabled() -> bool {
    std::env::var_os("RSOUP_SKIP_INTEGRATION").is_some()
}

/// Mock del gestor: acepta conexiones y responde según el protocolo v1.1.
struct MockGestor {
    listener: TcpListener,
    /// Secuencia de opcodes recibidos (para aseverar el flujo).
    ops: Arc<Mutex<Vec<u8>>>,
    /// Statements registrados por PREPARE (nombre -> SQL).
    prepared: Arc<Mutex<HashMap<String, String>>>,
    /// Veces que se parseó un SQL (PREPARE); EXECUTE debe reutilizar el plan.
    parse_count: Arc<Mutex<u32>>,
}

impl MockGestor {
    async fn spawn() -> io::Result<(MockGestor, u16)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        Ok((
            MockGestor {
                listener,
                ops: Arc::new(Mutex::new(Vec::new())),
                prepared: Arc::new(Mutex::new(HashMap::new())),
                parse_count: Arc::new(Mutex::new(0)),
            },
            port,
        ))
    }

    async fn serve(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = match self.listener.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                let ops = self.ops.clone();
                let prepared = self.prepared.clone();
                let parse_count = self.parse_count.clone();
                tokio::spawn(async move {
                    let _ = handle_stream(&mut stream, ops, prepared, parse_count).await;
                });
            }
        })
    }

    /// Clona el Arc de opcodes para poder consultarlo tras mover el gestor a la tarea.
    fn ops_handle(&self) -> Arc<Mutex<Vec<u8>>> {
        self.ops.clone()
    }

    /// Clona el Arc del contador de parseos (para verificar reutilización de planes).
    fn parse_count_handle(&self) -> Arc<Mutex<u32>> {
        self.parse_count.clone()
    }
}

async fn handle_stream(
    stream: &mut TcpStream,
    ops: Arc<Mutex<Vec<u8>>>,
    prepared: Arc<Mutex<HashMap<String, String>>>,
    parse_count: Arc<Mutex<u32>>,
) -> io::Result<()> {
    loop {
        let frame = match read_frame(stream).await {
            Ok(frame) => frame,
            Err(_) => return Ok(()),
        };
        ops.lock().unwrap().push(frame.opcode);
        let response = respond(&frame, &prepared, &parse_count);
        stream.write_all(&response.encode()).await?;
        stream.flush().await?;
    }
}

async fn read_frame(stream: &mut TcpStream) -> io::Result<Frame> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).await?;
    let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).await?;
    let mut data = header.to_vec();
    data.extend_from_slice(&payload);
    Frame::decode(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

fn respond(
    frame: &Frame,
    prepared: &Arc<Mutex<HashMap<String, String>>>,
    parse_count: &Arc<Mutex<u32>>,
) -> Frame {
    match frame.opcode {
        OP_PING => Frame {
            opcode: OP_PONG,
            payload: Vec::new(),
        },
        OP_BEGIN | OP_COMMIT | OP_ROLLBACK => Frame {
            opcode: OP_OK,
            payload: 0u32.to_be_bytes().to_vec(),
        },
        OP_PREPARE => match decode_prepare(&frame.payload) {
            Ok((name, sql)) => {
                prepared.lock().unwrap().insert(name, sql);
                *parse_count.lock().unwrap() += 1;
                Frame {
                    opcode: OP_OK,
                    payload: 0u32.to_be_bytes().to_vec(),
                }
            }
            Err(error) => Frame {
                opcode: OP_ERROR,
                payload: encode_error(rsoup::protocol::ERR_PARSE, &error.to_string()),
            },
        },
        OP_EXECUTE => match decode_execute(&frame.payload) {
            Ok((name, params)) => {
                if prepared.lock().unwrap().contains_key(&name) {
                    let rs = execute_result(&params);
                    Frame {
                        opcode: OP_RESULT,
                        payload: encode_resultset(&rs),
                    }
                } else {
                    Frame {
                        opcode: OP_ERROR,
                        payload: encode_error(
                            rsoup::protocol::ERR_RECORD_NOT_FOUND,
                            "prepared statement not found",
                        ),
                    }
                }
            }
            Err(error) => Frame {
                opcode: OP_ERROR,
                payload: encode_error(rsoup::protocol::ERR_PARSE, &error.to_string()),
            },
        },
        OP_DEALLOCATE => match decode_deallocate(&frame.payload) {
            Ok(name) => {
                prepared.lock().unwrap().remove(&name);
                Frame {
                    opcode: OP_OK,
                    payload: 0u32.to_be_bytes().to_vec(),
                }
            }
            Err(error) => Frame {
                opcode: OP_ERROR,
                payload: encode_error(rsoup::protocol::ERR_PARSE, &error.to_string()),
            },
        },
        OP_QUERY => {
            let sql =
                String::from_utf8_lossy(&frame.payload[4.min(frame.payload.len())..]).to_string();
            if sql.contains("BOGUS") {
                Frame {
                    opcode: OP_ERROR,
                    payload: encode_error(rsoup::protocol::ERR_PARSE, "syntax error"),
                }
            } else if sql.contains("INSERT") {
                Frame {
                    opcode: OP_OK,
                    payload: 1u32.to_be_bytes().to_vec(),
                }
            } else {
                let rs = ResultSet {
                    columns: vec![
                        Column::native("id", TYPE_NAME_INT, 0),
                        Column::native("balance", TYPE_NAME_INT, 0),
                    ],
                    rows: vec![
                        vec![Value::Int(1), Value::Int(100)],
                        vec![Value::Int(2), Value::Int(200)],
                    ],
                };
                Frame {
                    opcode: OP_RESULT,
                    payload: encode_resultset(&rs),
                }
            }
        }
        _ => Frame {
            opcode: OP_ERROR,
            payload: encode_error(rsoup::protocol::ERR_GENERIC, "unsupported opcode"),
        },
    }
}

/// Devuelve el primer parámetro tipado como una fila de resultado, para que el
/// test verifique que los parámetros (incluido VECTOR) sobreviven el viaje.
fn execute_result(params: &[Value]) -> ResultSet {
    let (type_name, value) = match params.first() {
        Some(Value::Int(i)) => (TYPE_NAME_INT, Value::Int(*i)),
        Some(Value::Float(f)) => (TYPE_NAME_FLOAT, Value::Float(*f)),
        Some(Value::Text(t)) => (TYPE_NAME_TEXT, Value::Text(t.clone())),
        Some(Value::Bool(b)) => (TYPE_NAME_BOOL, Value::Bool(*b)),
        Some(Value::Point(x, y)) => (TYPE_NAME_POINT, Value::Point(*x, *y)),
        Some(Value::Vector(values)) => (TYPE_NAME_VECTOR, Value::Vector(values.clone())),
        Some(Value::Unknown { bytes, .. }) => (
            TYPE_NAME_TEXT,
            Value::Text(format!("{} bytes", bytes.len())),
        ),
        Some(Value::Null) | None => (TYPE_NAME_INT, Value::Null),
    };
    ResultSet {
        columns: vec![Column::native("param", type_name, 0)],
        rows: vec![vec![value]],
    }
}

fn int(row: &[Value], index: usize) -> i32 {
    match row.get(index) {
        Some(Value::Int(value)) => *value,
        _ => -1,
    }
}

#[tokio::test]
async fn ping_against_mock_gestor() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let _handle = gestor.serve().await;
    let mut conn = Connection::connect("127.0.0.1", port).await.unwrap();
    conn.ping().await.unwrap();
}

#[tokio::test]
async fn query_decodes_resultset() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let _handle = gestor.serve().await;
    let mut conn = Connection::connect("127.0.0.1", port).await.unwrap();
    match conn.query("SELECT * FROM accounts").await.unwrap() {
        QueryResult::ResultSet(rs) => {
            assert_eq!(rs.columns[0].name, "id");
            assert_eq!(rs.rows.len(), 2);
            assert_eq!(int(&rs.rows[0], 0), 1);
            assert_eq!(int(&rs.rows[1], 1), 200);
        }
        QueryResult::Affected(_) => panic!("se esperaba ResultSet"),
    }
}

#[tokio::test]
async fn begin_query_commit_sequence() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let ops = gestor.ops_handle();
    let _handle = gestor.serve().await;
    let mut manager = TransactionManager::connect("127.0.0.1", port)
        .await
        .unwrap();
    manager.begin().await.unwrap();
    manager
        .query("INSERT INTO accounts VALUES (90, 1000)")
        .await
        .unwrap();
    manager.commit().await.unwrap();
    assert!(!manager.in_transaction());
    let seen = ops.lock().unwrap().clone();
    assert_eq!(seen, vec![OP_BEGIN, OP_QUERY, OP_COMMIT]);
}

#[tokio::test]
async fn begin_query_rollback_sequence() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let ops = gestor.ops_handle();
    let _handle = gestor.serve().await;
    let mut manager = TransactionManager::connect("127.0.0.1", port)
        .await
        .unwrap();
    manager.begin().await.unwrap();
    manager
        .query("INSERT INTO accounts VALUES (91, 2000)")
        .await
        .unwrap();
    manager.rollback().await.unwrap();
    assert!(!manager.in_transaction());
    let seen = ops.lock().unwrap().clone();
    assert_eq!(seen, vec![OP_BEGIN, OP_QUERY, OP_ROLLBACK]);
}

#[tokio::test]
async fn error_frame_becomes_driver_error() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let _handle = gestor.serve().await;
    let mut conn = Connection::connect("127.0.0.1", port).await.unwrap();
    let error = conn.query("BOGUS SQL").await.unwrap_err();
    assert_eq!(error.code, rsoup::protocol::ERR_PARSE);
    assert!(!error.message.is_empty());
}

#[tokio::test]
async fn transaction_guard_runs_and_commits() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let ops = gestor.ops_handle();
    let _handle = gestor.serve().await;
    let mut conn = Connection::connect("127.0.0.1", port).await.unwrap();
    let mut tx = Transaction::begin(&mut conn).await.unwrap();
    match tx
        .query("INSERT INTO accounts VALUES (5, 500)")
        .await
        .unwrap()
    {
        QueryResult::Affected(1) => {}
        _ => panic!("se esperaba Affected(1)"),
    }
    tx.commit().await.unwrap();
    let seen = ops.lock().unwrap().clone();
    assert_eq!(seen, vec![OP_BEGIN, OP_QUERY, OP_COMMIT]);
}

#[tokio::test]
async fn prepare_execute_deallocate_sequence_reuses_plan() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let ops = gestor.ops_handle();
    let parses = gestor.parse_count_handle();
    let _handle = gestor.serve().await;
    let mut manager = TransactionManager::connect("127.0.0.1", port)
        .await
        .unwrap();
    manager
        .prepare("buscar", "SELECT * FROM docs WHERE id = $1")
        .await
        .unwrap();
    match manager.execute("buscar", &[Value::Int(7)]).await.unwrap() {
        QueryResult::ResultSet(rs) => {
            assert_eq!(rs.columns[0].type_name, TYPE_NAME_INT);
            assert_eq!(rs.rows[0][0], Value::Int(7));
        }
        _ => panic!("se esperaba ResultSet"),
    }
    // El plan ya preparado se reutiliza: no se vuelve a parsear.
    manager.execute("buscar", &[Value::Int(8)]).await.unwrap();
    manager.deallocate("buscar").await.unwrap();
    let seen = ops.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![OP_PREPARE, OP_EXECUTE, OP_EXECUTE, OP_DEALLOCATE]
    );
    assert_eq!(*parses.lock().unwrap(), 1);
}

#[tokio::test]
async fn execute_with_vector_param_decodes() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let _handle = gestor.serve().await;
    let mut manager = TransactionManager::connect("127.0.0.1", port)
        .await
        .unwrap();
    manager
        .prepare(
            "vecino",
            "SELECT id FROM imagenes WHERE SEARCH(embedding, $1) USING HNSW",
        )
        .await
        .unwrap();
    let result = manager
        .execute("vecino", &[Value::Vector(vec![0.1, 0.2, 0.3])])
        .await
        .unwrap();
    match result {
        QueryResult::ResultSet(rs) => {
            assert!(rs.columns[0].is_vector());
            assert_eq!(rs.rows[0][0], Value::Vector(vec![0.1, 0.2, 0.3]));
        }
        _ => panic!("se esperaba ResultSet"),
    }
}

#[tokio::test]
async fn execute_unknown_statement_is_error() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let _handle = gestor.serve().await;
    let mut manager = TransactionManager::connect("127.0.0.1", port)
        .await
        .unwrap();
    let error = manager.execute("nope", &[]).await.unwrap_err();
    assert_eq!(error.code, rsoup::protocol::ERR_RECORD_NOT_FOUND);
}

#[tokio::test]
async fn transaction_guard_supports_prepared_statements() {
    if skip_if_disabled() {
        return;
    }
    let (gestor, port) = MockGestor::spawn().await.unwrap();
    let ops = gestor.ops_handle();
    let _handle = gestor.serve().await;
    let mut conn = Connection::connect("127.0.0.1", port).await.unwrap();
    let mut tx = Transaction::begin(&mut conn).await.unwrap();
    tx.prepare("s", "INSERT INTO t VALUES ($1)").await.unwrap();
    tx.execute("s", &[Value::Int(1)]).await.unwrap();
    tx.deallocate("s").await.unwrap();
    tx.commit().await.unwrap();
    let seen = ops.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![OP_BEGIN, OP_PREPARE, OP_EXECUTE, OP_DEALLOCATE, OP_COMMIT]
    );
}

#[tokio::test]
async fn error_variant_is_exported() {
    // Asegura que DriverError sea pública y usable como tipo de retorno.
    let _ = std::mem::size_of::<DriverError>();
}

#[test]
fn point_value_roundtrip() {
    let rs = ResultSet {
        columns: vec![
            Column::native("nombre", TYPE_NAME_TEXT, 0),
            Column::native("ubicacion", TYPE_NAME_POINT, 0),
        ],
        rows: vec![vec![
            Value::Text("Lima".into()),
            Value::Point(-77.03, -12.04),
        ]],
    };
    let encoded = encode_resultset(&rs);
    let decoded = decode_resultset(&encoded).unwrap();
    assert_eq!(decoded.rows[0][1], Value::Point(-77.03, -12.04));
}
