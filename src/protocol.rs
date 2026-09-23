//! Protocolo binario v1 entre el gestor SoupDB y el driver rsoup.
//!
//! Espeja exactamente `SoupDB/engine/transactions/protocol.py` (ver
//! `docs/protocolo.md`). Todo entero en big-endian.
//!
//! Frame: magic "SP" (2B) | version 0x01 (1B) | opcode (1B) |
//!        length u32 BE (4B) | payload (length bytes).

use std::fmt;

/// Magic del frame: "SP".
pub const MAGIC: [u8; 2] = *b"SP";
/// Versión del protocolo.
pub const VERSION: u8 = 0x01;
/// Tamaño del header sin payload.
pub const HEADER_SIZE: usize = 8;

// Opcodes de request
pub const OP_PING: u8 = 0x01;
pub const OP_BEGIN: u8 = 0x03;
pub const OP_COMMIT: u8 = 0x04;
pub const OP_ROLLBACK: u8 = 0x05;
pub const OP_QUERY: u8 = 0x06;
// Opcodes de response
pub const OP_PONG: u8 = 0x02;
pub const OP_OK: u8 = 0x10;
pub const OP_RESULT: u8 = 0x11;
pub const OP_ERROR: u8 = 0x12;

// Tags de valor
pub const TAG_NULL: u8 = 0x00;
pub const TAG_INT: u8 = 0x01;
pub const TAG_FLOAT: u8 = 0x02;
pub const TAG_TEXT: u8 = 0x03;
pub const TAG_BOOL: u8 = 0x04;

// Códigos de tipo de columna
pub const TYPE_INT: u8 = 0x01;
pub const TYPE_FLOAT: u8 = 0x02;
pub const TYPE_VARCHAR: u8 = 0x03;
pub const TYPE_TEXT: u8 = 0x04;
pub const TYPE_BOOL: u8 = 0x05;

// Códigos de error
pub const ERR_GENERIC: u8 = 0x00;
pub const ERR_PARSE: u8 = 0x01;
pub const ERR_EXECUTION: u8 = 0x02;
pub const ERR_TRANSACTION: u8 = 0x03;
pub const ERR_LOCK_TIMEOUT: u8 = 0x04;
pub const ERR_DEADLOCK: u8 = 0x05;

/// Error de protocolo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protocol error: {}", self.0)
    }
}

impl std::error::Error for ProtocolError {}

fn u16_be(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn u32_be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn i32_be(bytes: &[u8]) -> i32 {
    i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn f64_be(bytes: &[u8]) -> f64 {
    f64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// Un frame completo del wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Codifica el frame a bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.push(self.opcode);
        out.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Decodifica un frame completo desde un buffer.
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < HEADER_SIZE {
            return Err(ProtocolError(format!(
                "frame shorter than header: {} bytes",
                data.len()
            )));
        }
        if data[0..2] != MAGIC {
            return Err(ProtocolError(format!("bad magic {:?}", &data[0..2])));
        }
        let version = data[2];
        if version != VERSION {
            return Err(ProtocolError(format!("unsupported version {version}")));
        }
        let opcode = data[3];
        let length = u32_be(&data[4..8]) as usize;
        let payload = data
            .get(HEADER_SIZE..HEADER_SIZE + length)
            .ok_or_else(|| ProtocolError(format!("truncated frame: expected {length} bytes")))?;
        Ok(Frame {
            opcode,
            payload: payload.to_vec(),
        })
    }
}

/// Payload de QUERY.
pub fn encode_query(sql: &str) -> Vec<u8> {
    let bytes = sql.as_bytes();
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

pub fn decode_query(payload: &[u8]) -> Result<String, ProtocolError> {
    if payload.len() < 4 {
        return Err(ProtocolError("query payload too short".into()));
    }
    let length = u32_be(&payload[0..4]) as usize;
    let sql = std::str::from_utf8(&payload[4..4 + length])
        .map_err(|e| ProtocolError(format!("invalid utf-8 sql: {e}")))?;
    Ok(sql.to_string())
}

/// Payload de OK.
pub fn encode_ok(affected: u32) -> Vec<u8> {
    affected.to_be_bytes().to_vec()
}

pub fn decode_ok(payload: &[u8]) -> Result<u32, ProtocolError> {
    if payload.len() != 4 {
        return Err(ProtocolError("ok payload must be 4 bytes".into()));
    }
    Ok(u32_be(payload))
}

/// Payload de ERROR.
pub fn encode_error(code: u8, message: &str) -> Vec<u8> {
    let bytes = message.as_bytes();
    let mut out = Vec::with_capacity(3 + bytes.len());
    out.push(code);
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

pub fn decode_error(payload: &[u8]) -> Result<(u8, String), ProtocolError> {
    if payload.len() < 3 {
        return Err(ProtocolError("error payload too short".into()));
    }
    let code = payload[0];
    let length = u16_be(&payload[1..3]) as usize;
    let message = std::str::from_utf8(&payload[3..3 + length])
        .map_err(|e| ProtocolError(format!("invalid utf-8 error: {e}")))?
        .to_string();
    Ok((code, message))
}

/// Una columna declarada en un RESULT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub type_code: u8,
    pub length: u16,
}

/// Valor de una celda.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i32),
    Float(f64),
    Text(String),
    Bool(bool),
}

/// Resultado de una consulta (SELECT).
#[derive(Debug, Clone, PartialEq)]
pub struct ResultSet {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
}

fn encode_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => out.push(TAG_NULL),
        Value::Bool(b) => {
            out.push(TAG_BOOL);
            out.push(if *b { 1 } else { 0 });
        }
        Value::Int(i) => {
            out.push(TAG_INT);
            out.extend_from_slice(&i.to_be_bytes());
        }
        Value::Float(f) => {
            out.push(TAG_FLOAT);
            out.extend_from_slice(&f.to_be_bytes());
        }
        Value::Text(s) => {
            out.push(TAG_TEXT);
            let bytes = s.as_bytes();
            out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            out.extend_from_slice(bytes);
        }
    }
}

fn decode_value(payload: &[u8], offset: &mut usize) -> Result<Value, ProtocolError> {
    if *offset >= payload.len() {
        return Err(ProtocolError("value tag out of bounds".into()));
    }
    let tag = payload[*offset];
    *offset += 1;
    match tag {
        TAG_NULL => Ok(Value::Null),
        TAG_INT => {
            let bytes = payload
                .get(*offset..*offset + 4)
                .ok_or_else(|| ProtocolError("int value truncated".into()))?;
            *offset += 4;
            Ok(Value::Int(i32_be(bytes)))
        }
        TAG_FLOAT => {
            let bytes = payload
                .get(*offset..*offset + 8)
                .ok_or_else(|| ProtocolError("float value truncated".into()))?;
            *offset += 8;
            Ok(Value::Float(f64_be(bytes)))
        }
        TAG_TEXT => {
            let len_bytes = payload
                .get(*offset..*offset + 2)
                .ok_or_else(|| ProtocolError("text value truncated".into()))?;
            let length = u16_be(len_bytes) as usize;
            *offset += 2;
            let bytes = payload
                .get(*offset..*offset + length)
                .ok_or_else(|| ProtocolError("text value truncated".into()))?;
            *offset += length;
            let text = std::str::from_utf8(bytes)
                .map_err(|e| ProtocolError(format!("invalid utf-8 value: {e}")))?
                .to_string();
            Ok(Value::Text(text))
        }
        TAG_BOOL => {
            if *offset >= payload.len() {
                return Err(ProtocolError("bool value truncated".into()));
            }
            let b = payload[*offset] != 0;
            *offset += 1;
            Ok(Value::Bool(b))
        }
        other => Err(ProtocolError(format!("unknown value tag {other}"))),
    }
}

/// Payload de RESULT.
pub fn encode_resultset(rs: &ResultSet) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(rs.columns.len() as u16).to_be_bytes());
    for column in &rs.columns {
        let name = column.name.as_bytes();
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name);
        out.push(column.type_code);
        out.extend_from_slice(&column.length.to_be_bytes());
    }
    out.extend_from_slice(&(rs.rows.len() as u32).to_be_bytes());
    for row in &rs.rows {
        for value in row {
            encode_value(&mut out, value);
        }
    }
    out
}

pub fn decode_resultset(payload: &[u8]) -> Result<ResultSet, ProtocolError> {
    let mut offset = 0usize;
    if payload.len() < 2 {
        return Err(ProtocolError("resultset payload too short".into()));
    }
    let column_count = u16_be(&payload[0..2]) as usize;
    offset += 2;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        if offset + 2 > payload.len() {
            return Err(ProtocolError("column name length out of bounds".into()));
        }
        let name_len = u16_be(&payload[offset..offset + 2]) as usize;
        offset += 2;
        let name = std::str::from_utf8(
            payload
                .get(offset..offset + name_len)
                .ok_or_else(|| ProtocolError("column name truncated".into()))?,
        )
        .map_err(|e| ProtocolError(format!("invalid utf-8 column name: {e}")))?
        .to_string();
        offset += name_len;
        if offset + 3 > payload.len() {
            return Err(ProtocolError("column meta out of bounds".into()));
        }
        let type_code = payload[offset];
        offset += 1;
        let length = u16_be(&payload[offset..offset + 2]);
        offset += 2;
        columns.push(Column {
            name,
            type_code,
            length,
        });
    }
    if offset + 4 > payload.len() {
        return Err(ProtocolError("rows count out of bounds".into()));
    }
    let row_count = u32_be(&payload[offset..offset + 4]) as usize;
    offset += 4;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let mut row = Vec::with_capacity(columns.len());
        for _ in &columns {
            row.push(decode_value(payload, &mut offset)?);
        }
        rows.push(row);
    }
    Ok(ResultSet { columns, rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(opcode: u8, payload: Vec<u8>) -> Frame {
        Frame { opcode, payload }
    }

    #[test]
    fn frame_roundtrip() {
        let f = frame(OP_QUERY, encode_query("SELECT 1"));
        assert_eq!(Frame::decode(&f.encode()).unwrap(), f);
    }

    #[test]
    fn frame_layout() {
        let f = frame(OP_BEGIN, vec![]).encode();
        assert_eq!(&f[0..2], b"SP");
        assert_eq!(f[2], VERSION);
        assert_eq!(f[3], OP_BEGIN);
        assert_eq!(u32_be(&f[4..8]), 0);
    }

    #[test]
    fn bad_magic_rejected() {
        let data = b"XX\x01\x06\x00\x00\x00\x00".to_vec();
        assert!(Frame::decode(&data).is_err());
    }

    #[test]
    fn bad_version_rejected() {
        let mut data = MAGIC.to_vec();
        data.push(0x63);
        data.push(OP_PING);
        data.extend_from_slice(&[0, 0, 0, 0]);
        assert!(Frame::decode(&data).is_err());
    }

    #[test]
    fn query_roundtrip() {
        let sql = "SELECT * FROM papers WHERE anio = 2020";
        assert_eq!(decode_query(&encode_query(sql)).unwrap(), sql);
    }

    #[test]
    fn ok_roundtrip() {
        assert_eq!(decode_ok(&encode_ok(42)).unwrap(), 42);
    }

    #[test]
    fn error_roundtrip() {
        let (code, msg) = decode_error(&encode_error(ERR_PARSE, "boom")).unwrap();
        assert_eq!(code, ERR_PARSE);
        assert_eq!(msg, "boom");
    }

    #[test]
    fn resultset_roundtrip() {
        let rs = ResultSet {
            columns: vec![
                Column {
                    name: "id".into(),
                    type_code: TYPE_INT,
                    length: 0,
                },
                Column {
                    name: "anio".into(),
                    type_code: TYPE_INT,
                    length: 0,
                },
                Column {
                    name: "titulo".into(),
                    type_code: TYPE_TEXT,
                    length: 0,
                },
            ],
            rows: vec![
                vec![Value::Int(1), Value::Int(2019), Value::Text("VLDB".into())],
                vec![Value::Int(2), Value::Int(2020), Value::Null],
            ],
        };
        assert_eq!(decode_resultset(&encode_resultset(&rs)).unwrap(), rs);
    }

    #[test]
    fn resultset_all_types() {
        let rs = ResultSet {
            columns: vec![
                Column {
                    name: "i".into(),
                    type_code: TYPE_INT,
                    length: 0,
                },
                Column {
                    name: "f".into(),
                    type_code: TYPE_FLOAT,
                    length: 0,
                },
                Column {
                    name: "b".into(),
                    type_code: TYPE_BOOL,
                    length: 0,
                },
            ],
            rows: vec![vec![Value::Int(-5), Value::Float(1.5), Value::Bool(true)]],
        };
        let decoded = decode_resultset(&encode_resultset(&rs)).unwrap();
        assert_eq!(decoded.rows[0][1], Value::Float(1.5));
        assert_eq!(decoded.rows[0][2], Value::Bool(true));
    }
}
