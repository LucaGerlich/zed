//! Integration tests for pgblade-postgres.
//!
//! These tests require a running Postgres instance.
//! Start with: ./scripts/test-db.sh start
//!
//! Connection: localhost:15432, user=pgblade_test, password=pgblade_test, db=pgblade_test

use std::sync::Arc;

use pgblade_core::connection::{ConnectionId, ConnectionProfile, Environment, SslMode};
use pgblade_core::driver::{DatabaseDriver, DatabaseSession};
use pgblade_core::error::{ConnectionError, QueryError};
use pgblade_core::result::CellValue;
use pgblade_postgres::PostgresDriver;
use tokio::runtime::Runtime;

fn test_runtime() -> Arc<Runtime> {
    Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap(),
    )
}

fn test_profile() -> ConnectionProfile {
    ConnectionProfile {
        id: ConnectionId::new(),
        name: "test".to_string(),
        host: "localhost".to_string(),
        port: 15432,
        database: "pgblade_test".to_string(),
        username: "pgblade_test".to_string(),
        environment: Environment::Local,
        ssl_mode: SslMode::Disable,
        read_only_default: false,
    }
}

fn test_password() -> &'static str {
    "pgblade_test"
}

/// Helper to check if the test database is available.
/// Skips tests gracefully if Docker isn't running.
fn require_test_db(runtime: &Arc<Runtime>) -> bool {
    let profile = test_profile();
    let driver = PostgresDriver::new(runtime.clone());

    let result = runtime.block_on(async { driver.connect(&profile, test_password()).await });

    if result.is_err() {
        eprintln!("SKIPPING: test database not available. Run: ./scripts/test-db.sh start");
        return false;
    }
    true
}

#[test]
fn connects_with_valid_credentials() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let profile = test_profile();

    let result = runtime.block_on(async { driver.connect(&profile, test_password()).await });

    let session = result.expect("should connect successfully");
    assert!(
        session.server_version().contains("PostgreSQL"),
        "server version should contain 'PostgreSQL', got: {}",
        session.server_version()
    );
}

#[test]
fn fails_with_wrong_password() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let profile = test_profile();

    let result = runtime.block_on(async { driver.connect(&profile, "wrong_password").await });

    let err = result.err().expect("should fail with wrong password");
    assert!(
        matches!(err, ConnectionError::Authentication { .. }),
        "expected Authentication error, got: {err:?}"
    );
}

#[test]
fn fails_with_nonexistent_database() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let mut profile = test_profile();
    profile.database = "nonexistent_db_xyz".to_string();

    let result = runtime.block_on(async { driver.connect(&profile, test_password()).await });

    let err = result.err().expect("should fail with nonexistent database");
    assert!(
        matches!(err, ConnectionError::DatabaseNotFound { .. }),
        "expected DatabaseNotFound error, got: {err:?}"
    );
}

#[test]
fn executes_select_query() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let profile = test_profile();

    let result = runtime.block_on(async {
        let session = driver.connect(&profile, test_password()).await?;
        session
            .execute("SELECT 42 AS answer, 'hello' AS greeting")
            .await
            .map_err(|e| ConnectionError::Other {
                message: e.to_string(),
            })
    });

    let result_set = result.expect("query should succeed");

    assert_eq!(result_set.columns.len(), 2);
    assert_eq!(result_set.columns[0].name, "answer");
    assert_eq!(result_set.columns[1].name, "greeting");
    assert_eq!(result_set.total_rows, 1);
    assert_eq!(result_set.rows.len(), 1);

    match &result_set.rows[0][0] {
        CellValue::Integer(v) => assert_eq!(*v, 42),
        other => panic!("expected Integer(42), got: {other:?}"),
    }

    match &result_set.rows[0][1] {
        CellValue::Text(v) => assert_eq!(v, "hello"),
        other => panic!("expected Text(\"hello\"), got: {other:?}"),
    }
}

#[test]
fn returns_structured_error_on_invalid_sql() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let profile = test_profile();

    let result = runtime.block_on(async {
        let session = driver.connect(&profile, test_password()).await.unwrap();
        session.execute("SELECT * FROM nonexistent_table_xyz").await
    });

    let err = result.expect_err("query should fail");
    match err {
        QueryError::Postgres {
            code,
            severity,
            message,
            ..
        } => {
            assert_eq!(code, "42P01", "should be 'undefined_table' error code");
            assert_eq!(severity, "ERROR");
            assert!(
                message.contains("nonexistent_table_xyz"),
                "message should mention the table name: {message}"
            );
        }
        other => panic!("expected Postgres error, got: {other:?}"),
    }
}

#[test]
fn cancels_long_running_query() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let profile = test_profile();

    let result = runtime.block_on(async {
        let session: Arc<dyn DatabaseSession> =
            Arc::from(driver.connect(&profile, test_password()).await.unwrap());

        let session_for_cancel = session.clone();

        // Spawn a task that cancels after 500ms
        let cancel_handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            session_for_cancel.cancel().await
        });

        // Execute a query that takes 10 seconds
        let query_result = session.execute("SELECT pg_sleep(10)").await;

        // Wait for the cancel task to complete
        let _ = cancel_handle.await;

        query_result
    });

    // The query should have been cancelled
    let err = result.expect_err("query should be cancelled");
    match &err {
        QueryError::Postgres { code, .. } => {
            assert_eq!(code, "57014", "should be 'query_canceled' error code");
        }
        QueryError::Other { message } => {
            assert!(
                message.contains("cancel") || message.contains("closed"),
                "error should indicate cancellation: {message}"
            );
        }
        other => panic!("expected cancellation error, got: {other:?}"),
    }
}

#[test]
fn read_only_mode_blocks_writes() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let mut profile = test_profile();
    profile.read_only_default = true;

    let result = runtime.block_on(async {
        let session = driver
            .connect(&profile, test_password())
            .await
            .expect("should connect");

        // First create a test table (this should fail in read-only mode)
        session
            .execute("CREATE TABLE IF NOT EXISTS test_readonly (id int)")
            .await
    });

    let err = result.expect_err("write should be blocked in read-only mode");
    match &err {
        QueryError::Postgres { code, .. } => {
            // 25006 = read_only_sql_transaction
            assert_eq!(
                code, "25006",
                "should be 'read_only_sql_transaction' error: {err:?}"
            );
        }
        other => panic!("expected Postgres read-only error, got: {other:?}"),
    }
}

#[test]
fn handles_multiple_result_rows() {
    let runtime = test_runtime();
    if !require_test_db(&runtime) {
        return;
    }

    let driver = PostgresDriver::new(runtime.clone());
    let profile = test_profile();

    let result = runtime.block_on(async {
        let session = driver.connect(&profile, test_password()).await.unwrap();
        session
            .execute("SELECT generate_series(1, 100) AS num")
            .await
    });

    let result_set = result.expect("query should succeed");
    assert_eq!(result_set.total_rows, 100);
    assert_eq!(result_set.rows.len(), 100);

    // First row should be 1, last should be 100
    match &result_set.rows[0][0] {
        CellValue::Integer(v) => assert_eq!(*v, 1),
        other => panic!("expected Integer(1), got: {other:?}"),
    }
    match &result_set.rows[99][0] {
        CellValue::Integer(v) => assert_eq!(*v, 100),
        other => panic!("expected Integer(100), got: {other:?}"),
    }
}
