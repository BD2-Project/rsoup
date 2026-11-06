//! Protocolo binario v1.1 entre el gestor SoupDB y el driver rsoup.
//!
//! Espeja el contrato de `SoupDB/engine/transactions/protocol.py` (ver
//! `docs/protocolo.md`). Todo entero en big-endian.
//!
//! Frame: magic "SP" (2B) | version 0x01 (1B) | opcode (1B) |
//!        length u32 BE (4B) | payload (length bytes).
//!
//! El header de columna de `RESULT` en v1.1 es:
//! `name | type_name | encoding | size | length`, donde `type_name` es un
//! string (p. ej. `"INT"`, `"VECTOR"` o un tipo extensible del gestor) y
//! `encoding` indica cómo interpretar la columna. Los tipos no nativos llegan
//! como valores opacos (`Value::Unknown`) sin romper la conexión.
//!
//! Los 6 tipos nativos conservan sus códigos y tags originales.

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
/// Prepared statements (v1.1).
pub const OP_PREPARE: u8 = 0x07;
pub const OP_EXECUTE: u8 = 0x08;
pub const OP_DEALLOCATE: u8 = 0x09;
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
pub const TAG_POINT: u8 = 0x05;
/// Vector de `f64` (tipo `VECTOR(n)`): `dim u16 BE` + `dim * f64 BE`.
pub const TAG_VECTOR: u8 = 0x06;
/// Valor opaco de un tipo extensible: `len u32 BE` + bytes.
pub const TAG_OPAQUE: u8 = 0x07;

// Códigos de tipo de columna v1 (legado). Se conservan para el decoder v1 y
// para mapear los nombres nativos.
pub const TYPE_INT: u8 = 0x01;
pub const TYPE_FLOAT: u8 = 0x02;
pub const TYPE_VARCHAR: u8 = 0x03;
pub const TYPE_TEXT: u8 = 0x04;
pub const TYPE_BOOL: u8 = 0x05;
pub const TYPE_POINT: u8 = 0x06;

// Encoding de columna (v1.1)
/// Tipo nativo del motor (los 6 de siempre).
pub const ENC_NATIVE: u8 = 0;
/// Tipo de tamaño fijo (p. ej. `VECTOR(n)`).
pub const ENC_FIXED: u8 = 1;
/// Tipo de tamaño variable (p. ej. `VARCHAR`).
pub const ENC_VARIABLE: u8 = 2;
/// Tipo desconocido para el cliente: se entrega como bytes opacos.
pub const ENC_OPAQUE: u8 = 3;

// Nombres de tipo nativos
pub const TYPE_NAME_INT: &str = "INT";
pub const TYPE_NAME_FLOAT: &str = "FLOAT";
pub const TYPE_NAME_VARCHAR: &str = "VARCHAR";
pub const TYPE_NAME_TEXT: &str = "TEXT";
pub const TYPE_NAME_BOOL: &str = "BOOL";
pub const TYPE_NAME_POINT: &str = "POINT";
/// Tipo vectorial de primera clase.
pub const TYPE_NAME_VECTOR: &str = "VECTOR";

// Códigos de error
pub const ERR_GENERIC: u8 = 0x00;
pub const ERR_PARSE: u8 = 0x01;
pub const ERR_EXECUTION: u8 = 0x02;
pub const ERR_TRANSACTION: u8 = 0x03;
pub const ERR_LOCK_TIMEOUT: u8 = 0x04;
pub const ERR_DEADLOCK: u8 = 0x05;
pub const ERR_RECORD_NOT_FOUND: u8 = 0x06;
pub const ERR_CONTRACT: u8 = 0x07;
pub const ERR_UNSUPPORTED: u8 = 0x08;

/// Devuelve el nombre nativo de un código de tipo v1.
pub fn type_name_from_code(code: u8) -> Option<&'static str> {
    match code {
        TYPE_INT => Some(TYPE_NAME_INT),
        TYPE_FLOAT => Some(TYPE_NAME_FLOAT),
        TYPE_VARCHAR => Some(TYPE_NAME_VARCHAR),
        TYPE_TEXT => Some(TYPE_NAME_TEXT),
        TYPE_BOOL => Some(TYPE_NAME_BOOL),
        TYPE_POINT => Some(TYPE_NAME_POINT),
        _ => None,
    }
}

/// Devuelve el código v1 de un nombre de tipo nativo (case-insensitive).
pub fn native_type_code(type_name: &str) -> Option<u8> {
    match type_name.to_ascii_uppercase().as_str() {
        TYPE_NAME_INT => Some(TYPE_INT),
        TYPE_NAME_FLOAT => Some(TYPE_FLOAT),
        TYPE_NAME_VARCHAR => Some(TYPE_VARCHAR),
        TYPE_NAME_TEXT => Some(TYPE_TEXT),
        TYPE_NAME_BOOL => Some(TYPE_BOOL),
        TYPE_NAME_POINT => Some(TYPE_POINT),
        _ => None,
    }
}

/// Nombre legible de un `encoding` de columna.
pub fn encoding_name(encoding: u8) -> &'static str {
    match encoding {
        ENC_NATIVE => "native",
        ENC_FIXED => "fixed",
        ENC_VARIABLE => "variable",
        ENC_OPAQUE => "opaque",
        _ => "unknown",
    }
}

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

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, ProtocolError> {
    let byte = *payload
        .get(*offset)
        .ok_or_else(|| ProtocolError("unexpected end of payload (u8)".into()))?;
    *offset += 1;
    Ok(byte)
}

fn read_u16(payload: &[u8], offset: &mut usize) -> Result<u16, ProtocolError> {
    let bytes = payload
        .get(*offset..*offset + 2)
        .ok_or_else(|| ProtocolError("unexpected end of payload (u16)".into()))?;
    *offset += 2;
    Ok(u16_be(bytes))
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, ProtocolError> {
    let bytes = payload
        .get(*offset..*offset + 4)
        .ok_or_else(|| ProtocolError("unexpected end of payload (u32)".into()))?;
    *offset += 4;
    Ok(u32_be(bytes))
}

fn read_bytes<'a>(
    payload: &'a [u8],
    offset: &mut usize,
    length: usize,
) -> Result<&'a [u8], ProtocolError> {
    let bytes = payload
        .get(*offset..*offset + length)
        .ok_or_else(|| ProtocolError(format!("unexpected end of payload ({length} bytes)")))?;
    *offset += length;
    Ok(bytes)
}

fn read_string(payload: &[u8], offset: &mut usize, length: usize) -> Result<String, ProtocolError> {
    let bytes = read_bytes(payload, offset, length)?;
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|e| ProtocolError(format!("invalid utf-8 string: {e}")))
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
    let mut offset = 0usize;
    let length = read_u32(payload, &mut offset)? as usize;
    read_string(payload, &mut offset, length)
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
    let mut offset = 0usize;
    let code = read_u8(payload, &mut offset)?;
    let length = read_u16(payload, &mut offset)? as usize;
    let message = read_string(payload, &mut offset, length)?;
    Ok((code, message))
}

// --- Prepared statements (v1.1) ----------------------------------------

/// Payload de PREPARE: `name u16-len + UTF-8 | sql u32-len + UTF-8`.
pub fn encode_prepare(name: &str, sql: &str) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let sql_bytes = sql.as_bytes();
    let mut out = Vec::with_capacity(2 + name_bytes.len() + 4 + sql_bytes.len());
    out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(&(sql_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(sql_bytes);
    out
}

pub fn decode_prepare(payload: &[u8]) -> Result<(String, String), ProtocolError> {
    let mut offset = 0usize;
    let name_len = read_u16(payload, &mut offset)? as usize;
    let name = read_string(payload, &mut offset, name_len)?;
    let sql_len = read_u32(payload, &mut offset)? as usize;
    let sql = read_string(payload, &mut offset, sql_len)?;
    Ok((name, sql))
}

/// Payload de EXECUTE: `name u16-len + UTF-8 | param_count u16 | params`.
pub fn encode_execute(name: &str, params: &[Value]) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let mut out = Vec::with_capacity(2 + name_bytes.len() + 2 + params.len() * 9);
    out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(&(params.len() as u16).to_be_bytes());
    for param in params {
        encode_value(&mut out, param);
    }
    out
}

pub fn decode_execute(payload: &[u8]) -> Result<(String, Vec<Value>), ProtocolError> {
    let mut offset = 0usize;
    let name_len = read_u16(payload, &mut offset)? as usize;
    let name = read_string(payload, &mut offset, name_len)?;
    let param_count = read_u16(payload, &mut offset)? as usize;
    let mut params = Vec::with_capacity(param_count);
    for _ in 0..param_count {
        // Los parámetros no llevan header de columna: los tipos opacos no
        // tienen nombre asociado en este punto.
        params.push(decode_value(payload, &mut offset, "")?);
    }
    Ok((name, params))
}

/// Payload de DEALLOCATE: `name u16-len + UTF-8`.
pub fn encode_deallocate(name: &str) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let mut out = Vec::with_capacity(2 + name_bytes.len());
    out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(name_bytes);
    out
}

pub fn decode_deallocate(payload: &[u8]) -> Result<String, ProtocolError> {
    let mut offset = 0usize;
    let name_len = read_u16(payload, &mut offset)? as usize;
    read_string(payload, &mut offset, name_len)
}

/// Una columna declarada en un RESULT (v1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// Nombre del tipo: `INT`, `FLOAT`, `VARCHAR`, `TEXT`, `BOOL`, `POINT`,
    /// `VECTOR` o cualquier tipo extensible registrado en el gestor.
    pub type_name: String,
    /// `ENC_NATIVE`, `ENC_FIXED`, `ENC_VARIABLE` o `ENC_OPAQUE`.
    pub encoding: u8,
    /// Tamaño en bytes para tipos de tamaño fijo; 0 en el resto.
    pub size: u32,
    /// Longitud declarada (p. ej. `VARCHAR(n)`); 0 en el resto.
    pub length: u16,
}

impl Column {
    pub fn new(
        name: impl Into<String>,
        type_name: impl Into<String>,
        encoding: u8,
        size: u32,
        length: u16,
    ) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            encoding,
            size,
            length,
        }
    }

    /// Columna de un tipo nativo del motor.
    pub fn native(name: impl Into<String>, type_name: impl Into<String>, length: u16) -> Self {
        Self::new(name, type_name, ENC_NATIVE, 0, length)
    }

    pub fn is_native(&self) -> bool {
        self.encoding == ENC_NATIVE
    }

    pub fn is_opaque(&self) -> bool {
        self.encoding == ENC_OPAQUE
    }

    pub fn is_vector(&self) -> bool {
        self.type_name.eq_ignore_ascii_case(TYPE_NAME_VECTOR)
    }
}

/// Valor de una celda.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i32),
    Float(f64),
    Text(String),
    Bool(bool),
    /// Coordenada geográfica: `x` es longitud e `y` latitud, igual que el
    /// `Point(x, y)` del motor.
    Point(f64, f64),
    /// Vector de `f64` (tipo `VECTOR(n)`).
    Vector(Vec<f64>),
    /// Valor de un tipo extensible que el driver no conoce: bytes crudos +
    /// nombre del tipo (tomado del header de columna).
    Unknown {
        type_name: String,
        bytes: Vec<u8>,
    },
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
        Value::Point(x, y) => {
            out.push(TAG_POINT);
            out.extend_from_slice(&x.to_be_bytes());
            out.extend_from_slice(&y.to_be_bytes());
        }
        Value::Vector(values) => {
            out.push(TAG_VECTOR);
            out.extend_from_slice(&(values.len() as u16).to_be_bytes());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        Value::Unknown { bytes, .. } => {
            out.push(TAG_OPAQUE);
            out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            out.extend_from_slice(bytes);
        }
    }
}

fn decode_value(
    payload: &[u8],
    offset: &mut usize,
    column_type_name: &str,
) -> Result<Value, ProtocolError> {
    let tag = read_u8(payload, offset)?;
    match tag {
        TAG_NULL => Ok(Value::Null),
        TAG_INT => {
            let bytes = read_bytes(payload, offset, 4)?;
            Ok(Value::Int(i32_be(bytes)))
        }
        TAG_FLOAT => {
            let bytes = read_bytes(payload, offset, 8)?;
            Ok(Value::Float(f64_be(bytes)))
        }
        TAG_TEXT => {
            let length = read_u16(payload, offset)? as usize;
            let bytes = read_bytes(payload, offset, length)?;
            let text = std::str::from_utf8(bytes)
                .map_err(|e| ProtocolError(format!("invalid utf-8 value: {e}")))?
                .to_string();
            Ok(Value::Text(text))
        }
        TAG_BOOL => Ok(Value::Bool(read_u8(payload, offset)? != 0)),
        TAG_POINT => {
            let bytes = read_bytes(payload, offset, 16)?;
            Ok(Value::Point(f64_be(&bytes[..8]), f64_be(&bytes[8..])))
        }
        TAG_VECTOR => {
            let dimension = read_u16(payload, offset)? as usize;
            let mut values = Vec::with_capacity(dimension);
            for _ in 0..dimension {
                let bytes = read_bytes(payload, offset, 8)?;
                values.push(f64_be(bytes));
            }
            Ok(Value::Vector(values))
        }
        TAG_OPAQUE => {
            let length = read_u32(payload, offset)? as usize;
            let bytes = read_bytes(payload, offset, length)?.to_vec();
            Ok(Value::Unknown {
                type_name: column_type_name.to_string(),
                bytes,
            })
        }
        other => Err(ProtocolError(format!("unknown value tag {other}"))),
    }
}

/// Payload de RESULT (v1.1).
pub fn encode_resultset(rs: &ResultSet) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(rs.columns.len() as u16).to_be_bytes());
    for column in &rs.columns {
        let name = column.name.as_bytes();
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name);
        let type_name = column.type_name.as_bytes();
        out.extend_from_slice(&(type_name.len() as u16).to_be_bytes());
        out.extend_from_slice(type_name);
        out.push(column.encoding);
        out.extend_from_slice(&column.size.to_be_bytes());
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

/// Decodifica un RESULT con el header de columnas v1.1.
pub fn decode_resultset(payload: &[u8]) -> Result<ResultSet, ProtocolError> {
    let mut offset = 0usize;
    let column_count = read_u16(payload, &mut offset)? as usize;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        let name_len = read_u16(payload, &mut offset)? as usize;
        let name = read_string(payload, &mut offset, name_len)?;
        let type_name_len = read_u16(payload, &mut offset)? as usize;
        let type_name = read_string(payload, &mut offset, type_name_len)?;
        let encoding = read_u8(payload, &mut offset)?;
        let size = read_u32(payload, &mut offset)?;
        let length = read_u16(payload, &mut offset)?;
        columns.push(Column {
            name,
            type_name,
            encoding,
            size,
            length,
        });
    }
    let row_count = read_u32(payload, &mut offset)? as usize;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            row.push(decode_value(payload, &mut offset, &column.type_name)?);
        }
        rows.push(row);
    }
    Ok(ResultSet { columns, rows })
}

/// Decodifica un RESULT autodetectando el header: intenta v1.1 y, si falla,
/// cae al formato v1 legado. Permite que el driver hable con gestores previos
/// a v1.1 durante la transición.
pub fn decode_resultset_auto(payload: &[u8]) -> Result<ResultSet, ProtocolError> {
    match decode_resultset(payload) {
        Ok(resultset) => Ok(resultset),
        Err(v11_error) => decode_resultset_v1(payload).map_err(|v1_error| {
            ProtocolError(format!(
                "cannot decode RESULT as v1.1 ({v11_error}) nor v1 ({v1_error})"
            ))
        }),
    }
}

/// Codifica un RESULT con el header de columnas v1 (legado).
pub fn encode_resultset_v1(rs: &ResultSet) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(rs.columns.len() as u16).to_be_bytes());
    for column in &rs.columns {
        let name = column.name.as_bytes();
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name);
        let code = native_type_code(&column.type_name).unwrap_or(TYPE_INT);
        out.push(code);
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

/// Decodifica un RESULT con el header de columnas v1 (legado, `type_code`).
///
/// Se conserva para poder hablar con gestores previos a v1.1 durante la
/// transición. Los tipos nativos se mapean a su nombre (`INT`, `FLOAT`, ...) y
/// `encoding = ENC_NATIVE`.
pub fn decode_resultset_v1(payload: &[u8]) -> Result<ResultSet, ProtocolError> {
    let mut offset = 0usize;
    let column_count = read_u16(payload, &mut offset)? as usize;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        let name_len = read_u16(payload, &mut offset)? as usize;
        let name = read_string(payload, &mut offset, name_len)?;
        let type_code = read_u8(payload, &mut offset)?;
        let length = read_u16(payload, &mut offset)?;
        let type_name = type_name_from_code(type_code)
            .unwrap_or("UNKNOWN")
            .to_string();
        columns.push(Column {
            name,
            type_name,
            encoding: ENC_NATIVE,
            size: 0,
            length,
        });
    }
    let row_count = read_u32(payload, &mut offset)? as usize;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            row.push(decode_value(payload, &mut offset, &column.type_name)?);
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
    fn error_codes_aligned_with_gestor() {
        assert_eq!(ERR_RECORD_NOT_FOUND, 0x06);
        assert_eq!(ERR_CONTRACT, 0x07);
        assert_eq!(ERR_UNSUPPORTED, 0x08);
    }

    #[test]
    fn resultset_roundtrip() {
        let rs = ResultSet {
            columns: vec![
                Column::native("id", TYPE_NAME_INT, 0),
                Column::native("anio", TYPE_NAME_INT, 0),
                Column::native("titulo", TYPE_NAME_TEXT, 0),
            ],
            rows: vec![
                vec![Value::Int(1), Value::Int(2019), Value::Text("VLDB".into())],
                vec![Value::Int(2), Value::Int(2020), Value::Null],
            ],
        };
        assert_eq!(decode_resultset(&encode_resultset(&rs)).unwrap(), rs);
    }

    #[test]
    fn resultset_with_point_column() {
        // Sin esto el panel de mapa no recibe coordenadas: el motor resuelve la
        // consulta espacial pero el cliente no puede decodificar la geometría.
        let rs = ResultSet {
            columns: vec![
                Column::native("id", TYPE_NAME_INT, 0),
                Column::native("ubicacion", TYPE_NAME_POINT, 0),
            ],
            rows: vec![
                vec![Value::Int(1), Value::Point(-77.0428, -12.0464)],
                vec![Value::Int(2), Value::Null],
            ],
        };
        let decoded = decode_resultset(&encode_resultset(&rs)).unwrap();
        assert_eq!(decoded, rs);
        assert_eq!(decoded.rows[0][1], Value::Point(-77.0428, -12.0464));
    }

    #[test]
    fn point_tag_differs_from_float() {
        // Un punto no debe confundirse con dos flotantes sueltos al decodificar.
        assert_ne!(TAG_POINT, TAG_FLOAT);
        assert_ne!(TYPE_POINT, TYPE_FLOAT);
    }

    #[test]
    fn resultset_all_types() {
        let rs = ResultSet {
            columns: vec![
                Column::native("i", TYPE_NAME_INT, 0),
                Column::native("f", TYPE_NAME_FLOAT, 0),
                Column::native("b", TYPE_NAME_BOOL, 0),
            ],
            rows: vec![vec![Value::Int(-5), Value::Float(1.5), Value::Bool(true)]],
        };
        let decoded = decode_resultset(&encode_resultset(&rs)).unwrap();
        assert_eq!(decoded.rows[0][1], Value::Float(1.5));
        assert_eq!(decoded.rows[0][2], Value::Bool(true));
    }

    #[test]
    fn resultset_with_vector_column() {
        let rs = ResultSet {
            columns: vec![
                Column::native("id", TYPE_NAME_INT, 0),
                Column::new("embedding", TYPE_NAME_VECTOR, ENC_FIXED, 3 * 8, 0),
            ],
            rows: vec![
                vec![Value::Int(1), Value::Vector(vec![0.1, 0.2, 0.3])],
                vec![Value::Int(2), Value::Vector(vec![-1.0, 0.0, 1.0])],
            ],
        };
        let decoded = decode_resultset(&encode_resultset(&rs)).unwrap();
        assert_eq!(decoded, rs);
        assert_eq!(decoded.rows[0][1], Value::Vector(vec![0.1, 0.2, 0.3]));
        assert!(decoded.columns[1].is_vector());
        assert_eq!(decoded.columns[1].encoding, ENC_FIXED);
        assert_eq!(decoded.columns[1].size, 24);
    }

    #[test]
    fn resultset_with_unknown_opaque_column() {
        let rs = ResultSet {
            columns: vec![
                Column::native("id", TYPE_NAME_INT, 0),
                Column::new("custom", "GEOHASH_CUSTOM", ENC_OPAQUE, 0, 0),
            ],
            rows: vec![vec![
                Value::Int(7),
                Value::Unknown {
                    type_name: "GEOHASH_CUSTOM".into(),
                    bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
                },
            ]],
        };
        let decoded = decode_resultset(&encode_resultset(&rs)).unwrap();
        assert_eq!(decoded, rs);
        assert!(decoded.columns[1].is_opaque());
        match &decoded.rows[0][1] {
            Value::Unknown { type_name, bytes } => {
                assert_eq!(type_name, "GEOHASH_CUSTOM");
                assert_eq!(bytes, &vec![0xDE, 0xAD, 0xBE, 0xEF]);
            }
            other => panic!("se esperaba Unknown, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn legacy_v1_resultset_decodes() {
        let rs = ResultSet {
            columns: vec![
                Column::native("id", TYPE_NAME_INT, 0),
                Column::native("nombre", TYPE_NAME_TEXT, 0),
            ],
            rows: vec![vec![Value::Int(1), Value::Text("Lima".into())]],
        };
        let legacy = encode_resultset_v1(&rs);
        let decoded = decode_resultset_v1(&legacy).unwrap();
        assert_eq!(decoded, rs);
        // El header v1 no es decodificable como v1.1 de forma fiable.
        assert_eq!(decoded.columns[0].type_name, TYPE_NAME_INT);
        assert_eq!(decoded.columns[0].encoding, ENC_NATIVE);
    }

    #[test]
    fn decode_resultset_auto_handles_v1_and_v11() {
        let rs = ResultSet {
            columns: vec![
                Column::native("id", TYPE_NAME_INT, 0),
                Column::native("nombre", TYPE_NAME_TEXT, 0),
            ],
            rows: vec![vec![Value::Int(1), Value::Text("Lima".into())]],
        };
        // v1.1
        assert_eq!(decode_resultset_auto(&encode_resultset(&rs)).unwrap(), rs);
        // v1 legado (gestor previo a v1.1)
        assert_eq!(
            decode_resultset_auto(&encode_resultset_v1(&rs)).unwrap(),
            rs
        );
    }

    #[test]
    fn prepare_roundtrip() {
        let payload = encode_prepare("buscar", "SELECT * FROM docs WHERE id = $1");
        let (name, sql) = decode_prepare(&payload).unwrap();
        assert_eq!(name, "buscar");
        assert_eq!(sql, "SELECT * FROM docs WHERE id = $1");
    }

    #[test]
    fn execute_roundtrip_with_typed_params() {
        let params = vec![
            Value::Int(42),
            Value::Text("hola".into()),
            Value::Vector(vec![0.5, 0.25]),
            Value::Null,
        ];
        let payload = encode_execute("stmt", &params);
        let (name, decoded) = decode_execute(&payload).unwrap();
        assert_eq!(name, "stmt");
        assert_eq!(decoded, params);
    }

    #[test]
    fn deallocate_roundtrip() {
        let payload = encode_deallocate("stmt");
        assert_eq!(decode_deallocate(&payload).unwrap(), "stmt");
    }

    #[test]
    fn type_name_mapping() {
        for (code, name) in [
            (TYPE_INT, TYPE_NAME_INT),
            (TYPE_FLOAT, TYPE_NAME_FLOAT),
            (TYPE_VARCHAR, TYPE_NAME_VARCHAR),
            (TYPE_TEXT, TYPE_NAME_TEXT),
            (TYPE_BOOL, TYPE_NAME_BOOL),
            (TYPE_POINT, TYPE_NAME_POINT),
        ] {
            assert_eq!(type_name_from_code(code), Some(name));
            assert_eq!(native_type_code(name), Some(code));
        }
        assert_eq!(native_type_code("vector"), None);
    }

    #[test]
    fn encoding_helpers() {
        let column = Column::new("embedding", TYPE_NAME_VECTOR, ENC_FIXED, 128 * 8, 0);
        assert!(column.is_vector());
        assert!(!column.is_opaque());
        assert!(!column.is_native());
        assert_eq!(encoding_name(ENC_NATIVE), "native");
        assert_eq!(encoding_name(ENC_OPAQUE), "opaque");
    }
}
