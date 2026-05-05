use pgblade_core::result::{CellValue, ColumnMeta};
use tokio_postgres::Row;
use tokio_postgres::types::Type;

/// Extract column metadata from a prepared statement's columns.
pub fn extract_columns(columns: &[tokio_postgres::Column]) -> Vec<ColumnMeta> {
    columns
        .iter()
        .map(|col| ColumnMeta {
            name: col.name().to_string(),
            type_name: col.type_().name().to_string(),
            nullable: true, // tokio-postgres doesn't expose nullability from Statement
        })
        .collect()
}

/// Convert a single row into a Vec<CellValue>, one per column.
///
/// Each column is converted based on its Postgres type OID.
/// If a type isn't explicitly handled, it falls back to text representation.
pub fn convert_row(row: &Row) -> Vec<CellValue> {
    let columns = row.columns();
    (0..columns.len())
        .map(|i| convert_cell(row, i, columns[i].type_()))
        .collect()
}

/// Convert a single cell value based on its Postgres type.
///
/// TODO: This is a contribution point for the user.
/// The decision of how each Postgres type maps to CellValue
/// is a UX choice. For example:
///   - Should `numeric` be Float (lossy) or Text (exact)?
///   - Should `jsonb` be Json or Text?
///   - Should arrays be represented as Array or Text?
fn convert_cell(row: &Row, idx: usize, pg_type: &Type) -> CellValue {
    convert_cell_basic(row, idx, pg_type)
}

/// Basic type conversion — gets us running. User can refine later.
fn convert_cell_basic(row: &Row, idx: usize, pg_type: &Type) -> CellValue {
    match *pg_type {
        Type::BOOL => row
            .try_get::<_, Option<bool>>(idx)
            .ok()
            .flatten()
            .map(CellValue::Boolean)
            .unwrap_or(CellValue::Null),

        Type::INT2 => row
            .try_get::<_, Option<i16>>(idx)
            .ok()
            .flatten()
            .map(|v| CellValue::Integer(v as i64))
            .unwrap_or(CellValue::Null),

        Type::INT4 => row
            .try_get::<_, Option<i32>>(idx)
            .ok()
            .flatten()
            .map(|v| CellValue::Integer(v as i64))
            .unwrap_or(CellValue::Null),

        Type::INT8 => row
            .try_get::<_, Option<i64>>(idx)
            .ok()
            .flatten()
            .map(CellValue::Integer)
            .unwrap_or(CellValue::Null),

        Type::FLOAT4 => row
            .try_get::<_, Option<f32>>(idx)
            .ok()
            .flatten()
            .map(|v| CellValue::Float(v as f64))
            .unwrap_or(CellValue::Null),

        Type::FLOAT8 => row
            .try_get::<_, Option<f64>>(idx)
            .ok()
            .flatten()
            .map(CellValue::Float)
            .unwrap_or(CellValue::Null),

        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => row
            .try_get::<_, Option<String>>(idx)
            .ok()
            .flatten()
            .map(CellValue::Text)
            .unwrap_or(CellValue::Null),

        Type::BYTEA => row
            .try_get::<_, Option<Vec<u8>>>(idx)
            .ok()
            .flatten()
            .map(CellValue::Bytes)
            .unwrap_or(CellValue::Null),

        Type::JSON | Type::JSONB => row
            .try_get::<_, Option<serde_json::Value>>(idx)
            .ok()
            .flatten()
            .map(|v| CellValue::Json(v.to_string()))
            .unwrap_or(CellValue::Null),

        Type::UUID => row
            .try_get::<_, Option<uuid::Uuid>>(idx)
            .ok()
            .flatten()
            .map(|v| CellValue::Uuid(v.to_string()))
            .unwrap_or(CellValue::Null),

        // Fallback: try to get as String (Postgres can represent most types as text)
        _ => row
            .try_get::<_, Option<String>>(idx)
            .ok()
            .flatten()
            .map(CellValue::Text)
            .unwrap_or(CellValue::Null),
    }
}
