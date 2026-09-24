//! Integración del driver contra un gestor simulado (protocolo v1).
//!
//! Self-contained: un servidor tokio implementa el protocolo binario v1 y el
//! driver se conecta a él por un socket real. Para saltar estos tests
//! (intensivos de red) usar: `RSOUP_SKIP_INTEGRATION=1 cargo test`.

use std::io;
use std::sync::Arc;
use std::sync::Mutex;

use rsoup::connection_manager::{Connection, DriverError, QueryResult};
use rsoup::protocol::{
    encode_error, encode_resultset, Column, Frame, ResultSet, Value, OP_BEGIN, OP_COMMIT, OP_ERROR,
    OP_OK, OP_PING, OP_PONG, OP_QUERY, OP_RESULT, OP_ROLLBACK,
};
use rsoup::transaction_manager::{Transaction, TransactionManager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn skip_if_disabled() -> bool {
    std::env::var_os("RSOUP_SKIP_INTEGRATION").is_some()
}

/// Mock del gestor: acepta conexiones y responde según el protocolo v1.
struct MockGestor {
    listener: TcpListener,
    /// Secuencia de opcodes recibidos (para aseverar el flujo).
    ops: Arc<Mutex<Vec<u8>>>,
}

impl MockGestor {
    async fn spawn() -> io::Result<(MockGestor, u16)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        Ok((
            MockGestor {
                listener,
                ops: Arc::new(Mutex::new(Vec::new())),
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
                tokio::spawn(async move {
                    let _ = handle_stream(&mut stream, ops).await;
                });
            }
        })
    }

    /// Clona el Arc de opcodes para poder consultarlo tras mover el gestor a la tarea.
    fn ops_handle(&self) -> Arc<Mutex<Vec<u8>>> {
        self.ops.clone()
    }
}

async fn handle_stream(stream: &mut TcpStream, ops: Arc<Mutex<Vec<u8>>>) -> io::Result<()> {
    loop {
        let frame = match read_frame(stream).await {
            Ok(frame) => frame,
            Err(_) => return Ok(()),
        };
        ops.lock().unwrap().push(frame.opcode);
        let response = respond(&frame);
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

fn respond(frame: &Frame) -> Frame {
    match frame.opcode {
        OP_PING => Frame {
            opcode: OP_PONG,
            payload: Vec::new(),
        },
        OP_BEGIN | OP_COMMIT | OP_ROLLBACK => Frame {
            opcode: OP_OK,
            payload: 0u32.to_be_bytes().to_vec(),
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
                        Column {
                            name: "id".into(),
                            type_code: rsoup::protocol::TYPE_INT,
                            length: 0,
                        },
                        Column {
                            name: "balance".into(),
                            type_code: rsoup::protocol::TYPE_INT,
                            length: 0,
                        },
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
async fn error_variant_is_exported() {
    // Asegura que DriverError sea pública y usable como tipo de retorno.
    let _ = std::mem::size_of::<DriverError>();
}
