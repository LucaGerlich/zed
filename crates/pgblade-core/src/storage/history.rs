use chrono::DateTime;
use rusqlite::params;

use crate::query::{QueryClassification, QueryId, QueryRecord};

use super::{StorageError, StorageManager};

impl StorageManager {
    /// Persists a query execution record to the history table.
    pub fn save_query(&self, record: &QueryRecord) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO query_history
                (id, connection_id, sql_text, classification, executed_at, duration_ms, row_count, error, database_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id.0.to_string(),
                "",
                record.sql,
                record.classification.label(),
                record.executed_at.to_rfc3339(),
                record.duration_ms,
                record.row_count.map(|c| c as i64),
                record.error,
                record.database,
            ],
        )?;
        Ok(())
    }

    /// Loads the most recent query history records, ordered newest first.
    pub fn load_recent_history(&self, limit: usize) -> Result<Vec<QueryRecord>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, sql_text, classification, executed_at, duration_ms, row_count, error, database_name
             FROM query_history
             ORDER BY executed_at DESC
             LIMIT ?1",
        )?;

        let records = stmt
            .query_map(params![limit as i64], |row| {
                let id_str: String = row.get(0)?;
                let sql: String = row.get(1)?;
                let classification_str: String = row.get(2)?;
                let executed_at_str: String = row.get(3)?;
                let duration_ms: Option<i64> = row.get(4)?;
                let row_count: Option<i64> = row.get(5)?;
                let error: Option<String> = row.get(6)?;
                let database: String = row.get(7)?;

                let id = uuid::Uuid::parse_str(&id_str).unwrap_or_else(|_| uuid::Uuid::new_v4());
                let executed_at = DateTime::parse_from_rfc3339(&executed_at_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                Ok(QueryRecord {
                    id: QueryId(id),
                    sql,
                    classification: parse_classification(&classification_str),
                    executed_at,
                    duration_ms,
                    row_count: row_count.map(|c| c as u64),
                    error,
                    database,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }
}

/// Parses a classification label back into the `QueryClassification` enum.
fn parse_classification(s: &str) -> QueryClassification {
    match s {
        "Read" => QueryClassification::Read,
        "Write" => QueryClassification::Write,
        "Destructive" => QueryClassification::Destructive,
        "Transaction" => QueryClassification::TransactionControl,
        _ => QueryClassification::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_record() -> QueryRecord {
        QueryRecord {
            id: QueryId::new(),
            sql: "SELECT 1".to_string(),
            classification: QueryClassification::Read,
            executed_at: Utc::now(),
            duration_ms: Some(42),
            row_count: Some(1),
            error: None,
            database: "testdb".to_string(),
        }
    }

    #[test]
    fn saves_and_loads_query_record() {
        let storage = StorageManager::in_memory().unwrap();
        let record = test_record();

        storage.save_query(&record).unwrap();
        let history = storage.load_recent_history(10).unwrap();

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].sql, "SELECT 1");
        assert_eq!(history[0].classification, QueryClassification::Read);
        assert_eq!(history[0].duration_ms, Some(42));
        assert_eq!(history[0].row_count, Some(1));
        assert_eq!(history[0].database, "testdb");
    }

    #[test]
    fn respects_limit() {
        let storage = StorageManager::in_memory().unwrap();

        for i in 0..5 {
            let mut record = test_record();
            record.sql = format!("SELECT {i}");
            storage.save_query(&record).unwrap();
        }

        let history = storage.load_recent_history(3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn parses_classification_labels() {
        assert_eq!(parse_classification("Read"), QueryClassification::Read);
        assert_eq!(parse_classification("Write"), QueryClassification::Write);
        assert_eq!(
            parse_classification("Destructive"),
            QueryClassification::Destructive
        );
        assert_eq!(
            parse_classification("Transaction"),
            QueryClassification::TransactionControl
        );
        assert_eq!(
            parse_classification("garbage"),
            QueryClassification::Unknown
        );
    }
}
