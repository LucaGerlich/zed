use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::query::QueryClassification;

/// Classifies a SQL statement into a `QueryClassification` category.
///
/// Uses sqlparser to parse the SQL and inspects the first statement.
/// Returns `Unknown` if parsing fails or the statement is unrecognized.
pub fn classify_sql(sql: &str) -> QueryClassification {
    let dialect = PostgreSqlDialect {};
    let statements = match Parser::parse_sql(&dialect, sql) {
        Ok(stmts) => stmts,
        Err(_) => return QueryClassification::Unknown,
    };

    let Some(statement) = statements.first() else {
        return QueryClassification::Unknown;
    };

    use sqlparser::ast::Statement;

    match statement {
        // Read operations
        Statement::Query(_)
        | Statement::Explain { .. }
        | Statement::ShowTables { .. }
        | Statement::ShowColumns { .. }
        | Statement::ShowVariable { .. }
        | Statement::ShowCreate { .. } => QueryClassification::Read,

        // Write operations
        Statement::Insert(_)
        | Statement::Update { .. }
        | Statement::Copy { .. }
        | Statement::Merge { .. } => QueryClassification::Write,

        // DDL that creates — classified as Write
        Statement::CreateTable { .. }
        | Statement::CreateView { .. }
        | Statement::CreateIndex(_)
        | Statement::CreateSchema { .. } => QueryClassification::Write,

        // Destructive operations
        Statement::Drop { .. }
        | Statement::Truncate { .. }
        | Statement::Delete(_)
        | Statement::AlterTable { .. }
        | Statement::AlterIndex { .. } => QueryClassification::Destructive,

        // Transaction control
        Statement::StartTransaction { .. }
        | Statement::Commit { .. }
        | Statement::Rollback { .. }
        | Statement::Savepoint { .. }
        | Statement::SetVariable { .. } => QueryClassification::TransactionControl,

        // Everything else
        _ => QueryClassification::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_select_as_read() {
        assert_eq!(
            classify_sql("SELECT * FROM users"),
            QueryClassification::Read
        );
    }

    #[test]
    fn classifies_explain_as_read() {
        assert_eq!(classify_sql("EXPLAIN SELECT 1"), QueryClassification::Read);
    }

    #[test]
    fn classifies_insert_as_write() {
        assert_eq!(
            classify_sql("INSERT INTO users (name) VALUES ('Alice')"),
            QueryClassification::Write
        );
    }

    #[test]
    fn classifies_update_as_write() {
        assert_eq!(
            classify_sql("UPDATE users SET name = 'Bob' WHERE id = 1"),
            QueryClassification::Write
        );
    }

    #[test]
    fn classifies_create_table_as_write() {
        assert_eq!(
            classify_sql("CREATE TABLE items (id INT PRIMARY KEY)"),
            QueryClassification::Write
        );
    }

    #[test]
    fn classifies_drop_as_destructive() {
        assert_eq!(
            classify_sql("DROP TABLE users"),
            QueryClassification::Destructive
        );
    }

    #[test]
    fn classifies_delete_as_destructive() {
        assert_eq!(
            classify_sql("DELETE FROM users WHERE id = 1"),
            QueryClassification::Destructive
        );
    }

    #[test]
    fn classifies_truncate_as_destructive() {
        assert_eq!(
            classify_sql("TRUNCATE TABLE users"),
            QueryClassification::Destructive
        );
    }

    #[test]
    fn classifies_begin_as_transaction_control() {
        assert_eq!(
            classify_sql("BEGIN"),
            QueryClassification::TransactionControl
        );
    }

    #[test]
    fn classifies_commit_as_transaction_control() {
        assert_eq!(
            classify_sql("COMMIT"),
            QueryClassification::TransactionControl
        );
    }

    #[test]
    fn classifies_rollback_as_transaction_control() {
        assert_eq!(
            classify_sql("ROLLBACK"),
            QueryClassification::TransactionControl
        );
    }

    #[test]
    fn classifies_invalid_sql_as_unknown() {
        assert_eq!(
            classify_sql("NOT VALID SQL AT ALL !!!"),
            QueryClassification::Unknown
        );
    }

    #[test]
    fn classifies_empty_string_as_unknown() {
        assert_eq!(classify_sql(""), QueryClassification::Unknown);
    }
}
