use serde::{Deserialize, Serialize};

/// Metadata about a PostgreSQL schema (namespace).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub name: String,
    pub is_default: bool,
}

/// The kind of relation in a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableKind {
    Table,
    View,
    MaterializedView,
}

/// Metadata about a table, view, or materialized view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub kind: TableKind,
    pub row_estimate: Option<i64>,
}

impl TableInfo {
    /// Returns the fully qualified name: `schema.table`.
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}

/// Metadata about a single column in a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
    pub ordinal_position: i32,
}

/// Metadata about a constraint on a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintInfo {
    pub name: String,
    pub kind: ConstraintKind,
    pub columns: Vec<String>,
}

/// The kind of table constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintKind {
    PrimaryKey,
    ForeignKey,
    Unique,
    Check,
    Exclusion,
}

impl ConstraintKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PrimaryKey => "PRIMARY KEY",
            Self::ForeignKey => "FOREIGN KEY",
            Self::Unique => "UNIQUE",
            Self::Check => "CHECK",
            Self::Exclusion => "EXCLUSION",
        }
    }
}

/// Metadata about a foreign key relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}

/// Metadata about an index on a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub index_type: String, // btree, hash, gin, gist, etc.
}

/// Metadata about a trigger on a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub name: String,
    pub timing: String, // BEFORE, AFTER, INSTEAD OF
    pub event: String,  // INSERT, UPDATE, DELETE, TRUNCATE
}

/// Metadata about a stored function or procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub schema: String,
    pub arguments: String,
    pub return_type: String,
    pub language: String,
}

/// Metadata about a sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceInfo {
    pub name: String,
    pub schema: String,
    pub data_type: String,
}

/// A full schema tree representing the introspected database structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaTree {
    pub schemas: Vec<SchemaEntry>,
}

/// A schema with its child tables, views, functions, and sequences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub info: SchemaInfo,
    pub tables: Vec<TableEntry>,
    pub views: Vec<TableEntry>,
    pub materialized_views: Vec<TableEntry>,
    pub functions: Vec<FunctionInfo>,
    pub sequences: Vec<SequenceInfo>,
}

/// A table/view with its child columns, constraints, indexes, and triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableEntry {
    pub info: TableInfo,
    pub columns: Vec<ColumnInfo>,
    pub constraints: Vec<ConstraintInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
    pub triggers: Vec<TriggerInfo>,
}
