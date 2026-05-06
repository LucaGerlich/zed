mod connection_panel;
mod result_panel;

use editor::Editor;
use editor::actions::SelectAll;
use gpui::{App, AppContext as _, Context, Entity, Window, actions};
use language;
use workspace::Workspace;

pub use connection_panel::ConnectionPanel;
pub use result_panel::ResultPanel;

actions!(
    database_panel,
    [
        ExecuteQuery,
        ViewSessions,
        ExplainAnalyze,
        FormatSql,
        ViewHistory,
        Disconnect,
        KillBackend,
        SearchObjects,
        DatabaseInfo,
        TableSizes,
        SlowQueries,
        ViewLocks,
        IndexUsage
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, window: Option<&mut Window>, cx: &mut Context<Workspace>| {
            connection_panel::register(workspace);
            result_panel::register(workspace);

            // Register the ExecuteQuery action on the workspace
            workspace.register_action(|workspace, _: &ExecuteQuery, window, cx| {
                execute_query_action(workspace, window, cx);
            });

            // Register the ViewSessions action on the workspace
            workspace.register_action(|workspace, _: &ViewSessions, window, cx| {
                view_sessions_action(workspace, window, cx);
            });

            // Register the ExplainAnalyze action on the workspace
            workspace.register_action(|workspace, _: &ExplainAnalyze, window, cx| {
                explain_analyze_action(workspace, window, cx);
            });

            // Register the FormatSql action on the workspace
            workspace.register_action(|workspace, _: &FormatSql, window, cx| {
                format_sql_action(workspace, window, cx);
            });

            // Register the ViewHistory action on the workspace
            workspace.register_action(|workspace, _: &ViewHistory, window, cx| {
                view_history_action(workspace, window, cx);
            });

            // Register the Disconnect action on the workspace
            workspace.register_action(|workspace, _: &Disconnect, _window, cx| {
                if let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) {
                    conn_panel.update(cx, |panel, cx| panel.disconnect(cx));
                }
            });

            // Register the KillBackend action on the workspace
            workspace.register_action(|workspace, _: &KillBackend, window, cx| {
                kill_backend_action(workspace, window, cx);
            });

            // Register the SearchObjects action on the workspace
            workspace.register_action(|workspace, _: &SearchObjects, window, cx| {
                search_objects_action(workspace, window, cx);
            });

            // Register the DatabaseInfo action on the workspace
            workspace.register_action(|workspace, _: &DatabaseInfo, window, cx| {
                database_info_action(workspace, window, cx);
            });

            // Register the TableSizes action on the workspace
            workspace.register_action(|workspace, _: &TableSizes, window, cx| {
                table_sizes_action(workspace, window, cx);
            });

            // Register the SlowQueries action on the workspace
            workspace.register_action(|workspace, _: &SlowQueries, window, cx| {
                slow_queries_action(workspace, window, cx);
            });

            // Register the ViewLocks action on the workspace
            workspace.register_action(|workspace, _: &ViewLocks, window, cx| {
                view_locks_action(workspace, window, cx);
            });

            // Register the IndexUsage action on the workspace
            workspace.register_action(|workspace, _: &IndexUsage, window, cx| {
                index_usage_action(workspace, window, cx);
            });

            if let Some(window) = window {
                let workspace_weak = cx.weak_entity();
                let connection = cx.new(|cx| ConnectionPanel::new(workspace_weak, window, cx));
                workspace.add_panel(connection, window, cx);

                let results = cx.new(ResultPanel::new);
                workspace.add_panel(results, window, cx);
            }
        },
    )
    .detach();
}

fn get_sql_from_editor(editor: &Entity<Editor>, cx: &mut Context<Workspace>) -> Option<String> {
    let sql = editor.update(cx, |editor, cx| {
        let snapshot = editor.display_snapshot(cx);
        let selections = editor.selections.all::<language::Point>(&snapshot);
        let buffer = editor.buffer().read(cx).read(cx);

        // Use selected text if any selection has content, otherwise full buffer
        let selected: String = selections
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| buffer.text_for_range(s.start..s.end).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        if selected.trim().is_empty() {
            editor.text(cx)
        } else {
            selected
        }
    });

    if sql.trim().is_empty() {
        None
    } else {
        Some(sql)
    }
}

fn execute_query_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    // Get SQL from the active editor (selected text or full buffer)
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(sql) = get_sql_from_editor(&editor, cx) else {
        return;
    };

    // Get session from ConnectionPanel
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(session) = conn_panel.read(cx).session() else {
        tracing::warn!("ExecuteQuery: no active database connection");
        return;
    };
    let runtime = conn_panel.read(cx).runtime();

    // Save to query history
    conn_panel.update(cx, |panel, _cx| {
        panel.save_to_history(&sql);
    });

    // Open and update the result panel
    workspace.open_panel::<ResultPanel>(window, cx);

    let Some(result_panel) = workspace.panel::<ResultPanel>(cx) else {
        return;
    };

    result_panel.update(cx, |panel, cx| {
        panel.execute_query(sql, session, runtime, cx);
    });
}

fn view_sessions_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT pid, datname AS database, usename AS user, \
               application_name AS app, state, \
               CASE WHEN state = 'active' THEN (now() - query_start)::text ELSE '' END AS duration, \
               COALESCE(LEFT(query, 200), '') AS query, \
               COALESCE(wait_event_type || ': ' || wait_event, '') AS wait_event \
               FROM pg_stat_activity WHERE datname IS NOT NULL ORDER BY state DESC, query_start";

    execute_system_query(workspace, sql, window, cx);
}

fn explain_analyze_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    // Get SQL from the active editor (selected text or full buffer)
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(sql) = get_sql_from_editor(&editor, cx) else {
        return;
    };

    let explain_sql = format!("EXPLAIN (ANALYZE, COSTS, BUFFERS, FORMAT JSON) {sql}");

    // Get session from ConnectionPanel
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(session) = conn_panel.read(cx).session() else {
        tracing::warn!("ExplainAnalyze: no active database connection");
        return;
    };
    let runtime = conn_panel.read(cx).runtime();

    // Open and update the result panel with explain view
    workspace.open_panel::<ResultPanel>(window, cx);

    let Some(result_panel) = workspace.panel::<ResultPanel>(cx) else {
        return;
    };

    result_panel.update(cx, |panel, cx| {
        panel.execute_explain(explain_sql, session, runtime, cx);
    });
}

/// Format the SQL in the active editor using sqlformat.
fn format_sql_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let text = editor.read(cx).text(cx);
    if text.trim().is_empty() {
        return;
    }

    let options = sqlformat::FormatOptions {
        indent: sqlformat::Indent::Spaces(4),
        uppercase: true,
        lines_between_queries: 2,
        ..Default::default()
    };

    let formatted = sqlformat::format(&text, &sqlformat::QueryParams::None, options);

    editor.update(cx, |editor, cx| {
        editor.select_all(&SelectAll, window, cx);
        editor.insert(&formatted, window, cx);
    });
}

/// Show recent query history in the result panel.
fn view_history_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let history = conn_panel.read(cx).load_history();

    let columns = vec![
        pgblade_core::result::ColumnMeta {
            name: "Time".to_string(),
            type_name: "timestamp".to_string(),
            nullable: false,
        },
        pgblade_core::result::ColumnMeta {
            name: "SQL".to_string(),
            type_name: "text".to_string(),
            nullable: false,
        },
        pgblade_core::result::ColumnMeta {
            name: "Type".to_string(),
            type_name: "text".to_string(),
            nullable: false,
        },
        pgblade_core::result::ColumnMeta {
            name: "Duration".to_string(),
            type_name: "text".to_string(),
            nullable: true,
        },
        pgblade_core::result::ColumnMeta {
            name: "Database".to_string(),
            type_name: "text".to_string(),
            nullable: false,
        },
    ];

    let rows: Vec<Vec<pgblade_core::result::CellValue>> = history
        .iter()
        .map(|r| {
            vec![
                pgblade_core::result::CellValue::Text(
                    r.executed_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                ),
                pgblade_core::result::CellValue::Text(r.sql.chars().take(200).collect()),
                pgblade_core::result::CellValue::Text(r.classification.label().to_string()),
                r.duration_ms
                    .map(|d| pgblade_core::result::CellValue::Text(format!("{d}ms")))
                    .unwrap_or(pgblade_core::result::CellValue::Null),
                pgblade_core::result::CellValue::Text(r.database.clone()),
            ]
        })
        .collect();

    workspace.open_panel::<ResultPanel>(window, cx);
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        result_panel.update(cx, |panel, cx| {
            panel.show_results(columns, rows, cx);
        });
    }
}

/// Execute a system query (not from user editor) and display results in the ResultPanel.
fn execute_system_query(
    workspace: &mut Workspace,
    sql: &str,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(session) = conn_panel.read(cx).session() else {
        tracing::warn!("system query: no active database connection");
        return;
    };
    let runtime = conn_panel.read(cx).runtime();

    workspace.open_panel::<ResultPanel>(window, cx);

    let Some(result_panel) = workspace.panel::<ResultPanel>(cx) else {
        return;
    };

    result_panel.update(cx, |panel, cx| {
        panel.execute_query(sql.to_string(), session, runtime, cx);
    });
}

/// Search database objects (tables, columns, functions) matching the selected text.
fn search_objects_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(search_term) = get_sql_from_editor(&editor, cx) else {
        return;
    };
    let term = search_term.trim();
    if term.is_empty() {
        return;
    }

    // Sanitize the search term to prevent SQL injection
    let safe_term = term
        .replace('\'', "''")
        .replace('%', "\\%")
        .replace('_', "\\_");

    let sql = format!(
        "SELECT 'TABLE' as type, table_schema as schema, table_name as name, '' as detail \
         FROM information_schema.tables \
         WHERE table_name ILIKE '%{safe_term}%' AND table_schema NOT IN ('pg_catalog', 'information_schema') \
         UNION ALL \
         SELECT 'COLUMN', table_schema, table_name || '.' || column_name, data_type \
         FROM information_schema.columns \
         WHERE column_name ILIKE '%{safe_term}%' AND table_schema NOT IN ('pg_catalog', 'information_schema') \
         UNION ALL \
         SELECT 'FUNCTION', n.nspname, p.proname, pg_get_function_result(p.oid) \
         FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
         WHERE p.proname ILIKE '%{safe_term}%' AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
         ORDER BY type, name \
         LIMIT 50"
    );

    execute_system_query(workspace, &sql, window, cx);
}

/// Terminate a PostgreSQL backend by PID. Reads the PID from the active editor selection.
fn kill_backend_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(selected) = get_sql_from_editor(&editor, cx) else {
        tracing::warn!("KillBackend: no text selected");
        return;
    };
    let pid = selected.trim();

    if pid.parse::<i32>().is_err() {
        tracing::warn!("KillBackend: selected text is not a valid PID: {pid}");
        return;
    }

    let sql = format!("SELECT pg_terminate_backend({pid})");
    execute_system_query(workspace, &sql, window, cx);
}

/// Show database-level statistics (size, connections, collation).
fn database_info_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        pg_database.datname AS database, \
        pg_size_pretty(pg_database_size(pg_database.datname)) AS size, \
        (SELECT count(*) FROM pg_stat_activity WHERE datname = pg_database.datname) AS connections, \
        pg_database.datcollate AS collation, \
        pg_database.encoding AS encoding_id \
    FROM pg_database \
    WHERE datistemplate = false \
    ORDER BY pg_database_size(pg_database.datname) DESC";

    execute_system_query(workspace, sql, window, cx);
}

/// Show table sizes ordered by total size descending.
fn table_sizes_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "SELECT \
        schemaname || '.' || tablename AS table_name, \
        pg_size_pretty(pg_total_relation_size(schemaname || '.' || tablename)) AS total_size, \
        pg_size_pretty(pg_relation_size(schemaname || '.' || tablename)) AS data_size, \
        pg_size_pretty(pg_total_relation_size(schemaname || '.' || tablename) - pg_relation_size(schemaname || '.' || tablename)) AS index_size, \
        (SELECT reltuples::bigint FROM pg_class WHERE relname = tablename) AS estimated_rows \
    FROM pg_tables \
    WHERE schemaname NOT IN ('pg_catalog', 'information_schema') \
    ORDER BY pg_total_relation_size(schemaname || '.' || tablename) DESC \
    LIMIT 50";

    execute_system_query(workspace, sql, window, cx);
}

/// Show currently running (non-idle) queries ordered by duration.
fn slow_queries_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        pid, \
        now() - pg_stat_activity.query_start AS duration, \
        state, \
        usename AS user, \
        datname AS database, \
        LEFT(query, 300) AS query \
    FROM pg_stat_activity \
    WHERE state != 'idle' \
        AND query NOT ILIKE '%pg_stat_activity%' \
    ORDER BY duration DESC \
    LIMIT 20";

    execute_system_query(workspace, sql, window, cx);
}

/// Show blocked and blocking queries (lock contention).
fn view_locks_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "SELECT \
        blocked_locks.pid AS blocked_pid, \
        blocked_activity.usename AS blocked_user, \
        LEFT(blocked_activity.query, 200) AS blocked_query, \
        blocking_locks.pid AS blocking_pid, \
        blocking_activity.usename AS blocking_user, \
        LEFT(blocking_activity.query, 200) AS blocking_query \
    FROM pg_catalog.pg_locks blocked_locks \
    JOIN pg_catalog.pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_locks.pid \
    JOIN pg_catalog.pg_locks blocking_locks ON blocking_locks.locktype = blocked_locks.locktype \
        AND blocking_locks.database IS NOT DISTINCT FROM blocked_locks.database \
        AND blocking_locks.relation IS NOT DISTINCT FROM blocked_locks.relation \
        AND blocking_locks.page IS NOT DISTINCT FROM blocked_locks.page \
        AND blocking_locks.tuple IS NOT DISTINCT FROM blocked_locks.tuple \
        AND blocking_locks.virtualxid IS NOT DISTINCT FROM blocked_locks.virtualxid \
        AND blocking_locks.transactionid IS NOT DISTINCT FROM blocked_locks.transactionid \
        AND blocking_locks.classid IS NOT DISTINCT FROM blocked_locks.classid \
        AND blocking_locks.objid IS NOT DISTINCT FROM blocked_locks.objid \
        AND blocking_locks.objsubid IS NOT DISTINCT FROM blocked_locks.objsubid \
        AND blocking_locks.pid != blocked_locks.pid \
    JOIN pg_catalog.pg_stat_activity blocking_activity ON blocking_activity.pid = blocking_locks.pid \
    WHERE NOT blocked_locks.granted \
    ORDER BY blocked_activity.query_start";

    execute_system_query(workspace, sql, window, cx);
}

/// Show index usage statistics sorted by least-used indexes.
fn index_usage_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "SELECT \
        schemaname || '.' || relname AS table, \
        indexrelname AS index, \
        idx_scan AS scans, \
        pg_size_pretty(pg_relation_size(indexrelid)) AS size, \
        idx_tup_read AS tuples_read, \
        idx_tup_fetch AS tuples_fetched \
    FROM pg_stat_user_indexes \
    ORDER BY idx_scan ASC, pg_relation_size(indexrelid) DESC \
    LIMIT 30";

    execute_system_query(workspace, sql, window, cx);
}
