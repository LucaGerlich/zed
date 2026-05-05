/// Metadata for a single column in a result set.
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
}

/// A single cell value in a result row.
/// Extended with more variants as PgBlade supports more Postgres types.
#[derive(Debug, Clone)]
pub enum CellValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Json(String),
    Timestamp(String),
    Date(String),
    Time(String),
    Uuid(String),
    Array(Vec<CellValue>),
}

impl CellValue {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Render the cell as a display string for the result grid.
    pub fn display(&self) -> String {
        match self {
            Self::Null => "NULL".to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Integer(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Text(s) => s.clone(),
            Self::Bytes(b) => format!("\\x{}", hex_encode(b)),
            Self::Json(s) => s.clone(),
            Self::Timestamp(s) | Self::Date(s) | Self::Time(s) | Self::Uuid(s) => s.clone(),
            Self::Array(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("{{{}}}", inner.join(","))
            }
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A page of query results. The result grid renders one page at a time.
#[derive(Debug, Clone)]
pub struct ResultPage {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<CellValue>>,
    pub page_index: u32,
    pub has_more: bool,
}

/// A complete result set (may span multiple pages for display).
#[derive(Debug, Clone)]
pub struct ResultSet {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<CellValue>>,
    pub total_rows: u64,
}
