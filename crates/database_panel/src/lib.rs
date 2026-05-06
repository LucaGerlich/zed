mod connection_panel;
mod result_panel;
mod sql_completion;

use editor::Editor;
use editor::actions::SelectAll;
use gpui::{App, AppContext as _, ClipboardItem, Context, Entity, Window, actions};
use picker::Picker;
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
        IndexUsage,
        ERDiagram,
        ImportCsv,
        CompareSchemas,
        ViewExtensions,
        SqlComplete,
        ViewComments,
        DumpSchema,
        ViewPrivileges,
        ViewRoles,
        ViewTablespaces,
        ViewReplication,
        ViewDefinition,
        ColumnStats,
        VacuumProgress,
        CacheHitRatio,
        ViewSettings,
        SequenceValues,
        ViewDependencies,
        ConnectionStats,
        WalStatus,
        TableBloat,
        UnusedIndexes,
        MissingIndexes,
        ViewPartitions,
        AutovacuumStatus,
        ViewForeignTables,
        CreateFunction,
        CreateTrigger
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

            // Register the ERDiagram action on the workspace
            workspace.register_action(|workspace, _: &ERDiagram, window, cx| {
                er_diagram_action(workspace, window, cx);
            });

            // Register the ImportCsv action on the workspace
            workspace.register_action(|workspace, _: &ImportCsv, window, cx| {
                import_csv_action(workspace, window, cx);
            });

            // Register the CompareSchemas action on the workspace
            workspace.register_action(|workspace, _: &CompareSchemas, window, cx| {
                compare_schemas_action(workspace, window, cx);
            });

            // Register the ViewExtensions action on the workspace
            workspace.register_action(|workspace, _: &ViewExtensions, window, cx| {
                view_extensions_action(workspace, window, cx);
            });

            // Register the SqlComplete action on the workspace
            workspace.register_action(|workspace, _: &SqlComplete, window, cx| {
                sql_complete_action(workspace, window, cx);
            });

            // Register the ViewComments action on the workspace
            workspace.register_action(|workspace, _: &ViewComments, window, cx| {
                view_comments_action(workspace, window, cx);
            });

            // Register the DumpSchema action on the workspace
            workspace.register_action(|workspace, _: &DumpSchema, window, cx| {
                dump_schema_action(workspace, window, cx);
            });

            // Register the ViewPrivileges action on the workspace
            workspace.register_action(|workspace, _: &ViewPrivileges, window, cx| {
                view_privileges_action(workspace, window, cx);
            });

            // Register the ViewRoles action on the workspace
            workspace.register_action(|workspace, _: &ViewRoles, window, cx| {
                view_roles_action(workspace, window, cx);
            });

            // Register the ViewTablespaces action on the workspace
            workspace.register_action(|workspace, _: &ViewTablespaces, window, cx| {
                view_tablespaces_action(workspace, window, cx);
            });

            // Register the ViewReplication action on the workspace
            workspace.register_action(|workspace, _: &ViewReplication, window, cx| {
                view_replication_action(workspace, window, cx);
            });

            // Register the ViewDefinition action on the workspace
            workspace.register_action(|workspace, _: &ViewDefinition, window, cx| {
                view_definition_action(workspace, window, cx);
            });

            // Register the ColumnStats action on the workspace
            workspace.register_action(|workspace, _: &ColumnStats, window, cx| {
                column_stats_action(workspace, window, cx);
            });

            // Register the VacuumProgress action on the workspace
            workspace.register_action(|workspace, _: &VacuumProgress, window, cx| {
                vacuum_progress_action(workspace, window, cx);
            });

            // Register the CacheHitRatio action on the workspace
            workspace.register_action(|workspace, _: &CacheHitRatio, window, cx| {
                cache_hit_ratio_action(workspace, window, cx);
            });

            // Register the ViewSettings action on the workspace
            workspace.register_action(|workspace, _: &ViewSettings, window, cx| {
                view_settings_action(workspace, window, cx);
            });

            // Register the SequenceValues action on the workspace
            workspace.register_action(|workspace, _: &SequenceValues, window, cx| {
                sequence_values_action(workspace, window, cx);
            });

            // Register the ViewDependencies action on the workspace
            workspace.register_action(|workspace, _: &ViewDependencies, window, cx| {
                view_dependencies_action(workspace, window, cx);
            });

            // Register the ConnectionStats action on the workspace
            workspace.register_action(|workspace, _: &ConnectionStats, window, cx| {
                connection_stats_action(workspace, window, cx);
            });

            // Register the WalStatus action on the workspace
            workspace.register_action(|workspace, _: &WalStatus, window, cx| {
                wal_status_action(workspace, window, cx);
            });

            // Register the TableBloat action on the workspace
            workspace.register_action(|workspace, _: &TableBloat, window, cx| {
                table_bloat_action(workspace, window, cx);
            });

            // Register the UnusedIndexes action on the workspace
            workspace.register_action(|workspace, _: &UnusedIndexes, window, cx| {
                unused_indexes_action(workspace, window, cx);
            });

            // Register the MissingIndexes action on the workspace
            workspace.register_action(|workspace, _: &MissingIndexes, window, cx| {
                missing_indexes_action(workspace, window, cx);
            });

            // Register the ViewPartitions action on the workspace
            workspace.register_action(|workspace, _: &ViewPartitions, window, cx| {
                view_partitions_action(workspace, window, cx);
            });

            // Register the AutovacuumStatus action on the workspace
            workspace.register_action(|workspace, _: &AutovacuumStatus, window, cx| {
                autovacuum_status_action(workspace, window, cx);
            });

            // Register the ViewForeignTables action on the workspace
            workspace.register_action(|workspace, _: &ViewForeignTables, window, cx| {
                view_foreign_tables_action(workspace, window, cx);
            });

            // Register the CreateFunction action on the workspace
            workspace.register_action(|workspace, _: &CreateFunction, window, cx| {
                create_function_action(workspace, window, cx);
            });

            // Register the CreateTrigger action on the workspace
            workspace.register_action(|workspace, _: &CreateTrigger, window, cx| {
                create_trigger_action(workspace, window, cx);
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

/// Generate a Mermaid ER diagram from the introspected schema tree.
fn er_diagram_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(schema_tree) = conn_panel.read(cx).schema_tree() else {
        tracing::warn!("ERDiagram: no schema tree available (not connected or schema not loaded)");
        return;
    };

    let mut mermaid = String::from("erDiagram\n");

    for schema in &schema_tree.schemas {
        // Tables
        for table in &schema.tables {
            let table_name = format!("{}_{}", schema.info.name, table.info.name);
            mermaid.push_str(&format!("    {} {{\n", table_name));
            for col in &table.columns {
                let pk = if col.is_primary_key { " PK" } else { "" };
                let nullable = if col.nullable { " \"nullable\"" } else { "" };
                let type_name = col.data_type.replace(' ', "_");
                mermaid.push_str(&format!(
                    "        {} {}{}{}\n",
                    type_name, col.name, pk, nullable
                ));
            }
            mermaid.push_str("    }\n");
        }

        // Relationships from foreign keys
        for table in &schema.tables {
            let table_name = format!("{}_{}", schema.info.name, table.info.name);
            for fk in &table.foreign_keys {
                let ref_table = if fk.referenced_table.contains('.') {
                    fk.referenced_table.replace('.', "_")
                } else {
                    format!("{}_{}", schema.info.name, fk.referenced_table)
                };
                mermaid.push_str(&format!(
                    "    {} ||--o{{ {} : \"{}\"\n",
                    ref_table, table_name, fk.name
                ));
            }
        }
    }

    // Show in result panel
    workspace.open_panel::<ResultPanel>(window, cx);
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(mermaid, cx);
        });
    }
}

/// Import CSV data from the clipboard and generate INSERT statements.
fn import_csv_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(clipboard) = cx.read_from_clipboard() else {
        tracing::warn!("ImportCsv: clipboard is empty");
        return;
    };
    let Some(text) = clipboard.text() else {
        tracing::warn!("ImportCsv: clipboard has no text content");
        return;
    };
    if text.trim().is_empty() {
        tracing::warn!("ImportCsv: clipboard text is empty");
        return;
    }

    let mut lines = text.lines();
    let Some(header_line) = lines.next() else {
        return;
    };

    // Auto-detect delimiter (tab or comma)
    let delimiter = if header_line.contains('\t') {
        '\t'
    } else {
        ','
    };
    let headers: Vec<&str> = header_line.split(delimiter).map(|h| h.trim()).collect();

    let col_list = headers
        .iter()
        .map(|h| format!("\"{}\"", h))
        .collect::<Vec<_>>()
        .join(", ");

    let mut inserts = String::new();
    inserts.push_str(&format!(
        "-- Import {} columns: {}\n",
        headers.len(),
        col_list
    ));
    inserts.push_str("-- Replace 'your_table' with the target table name\n\n");

    let mut row_count = 0;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let values: Vec<&str> = line.split(delimiter).collect();
        let value_list: Vec<String> = values
            .iter()
            .map(|v| {
                let trimmed = v.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                    "NULL".to_string()
                } else {
                    format!("'{}'", trimmed.replace('\'', "''"))
                }
            })
            .collect();
        inserts.push_str(&format!(
            "INSERT INTO your_table ({col_list}) VALUES ({});\n",
            value_list.join(", ")
        ));
        row_count += 1;
    }

    inserts.push_str(&format!("\n-- {row_count} rows imported from clipboard\n"));

    cx.write_to_clipboard(ClipboardItem::new_string(inserts.clone()));

    // Show in result panel
    workspace.open_panel::<ResultPanel>(window, cx);
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(inserts, cx);
        });
    }
}

/// Show schema comparison data: tables with column, index, and FK counts.
fn compare_schemas_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        t.table_schema, \
        t.table_name, \
        (SELECT count(*) FROM information_schema.columns c \
            WHERE c.table_schema = t.table_schema AND c.table_name = t.table_name) AS column_count, \
        (SELECT count(*) FROM pg_indexes i \
            WHERE i.schemaname = t.table_schema AND i.tablename = t.table_name) AS index_count, \
        (SELECT count(*) FROM information_schema.table_constraints tc \
            WHERE tc.table_schema = t.table_schema AND tc.table_name = t.table_name \
            AND tc.constraint_type = 'FOREIGN KEY') AS fk_count \
    FROM information_schema.tables t \
    WHERE t.table_schema NOT IN ('pg_catalog', 'information_schema') \
    AND t.table_type = 'BASE TABLE' \
    ORDER BY t.table_schema, t.table_name";

    execute_system_query(workspace, sql, window, cx);
}

/// Show installed PostgreSQL extensions with their versions.
fn view_extensions_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        e.extname AS name, \
        e.extversion AS version, \
        n.nspname AS schema, \
        c.description \
    FROM pg_extension e \
    JOIN pg_namespace n ON n.oid = e.extnamespace \
    LEFT JOIN pg_description c ON c.objoid = e.oid \
    ORDER BY e.extname";

    execute_system_query(workspace, sql, window, cx);
}

/// Open the SQL completion picker populated with schema objects and SQL keywords.
fn sql_complete_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(schema_tree) = conn_panel.read(cx).schema_tree() else {
        tracing::warn!(
            "SqlComplete: no schema tree available (not connected or schema not loaded)"
        );
        return;
    };

    let items = sql_completion::build_completion_items(schema_tree);
    let workspace_weak = cx.weak_entity();

    workspace.toggle_modal(window, cx, |window, cx| {
        let delegate = sql_completion::SqlCompletionDelegate::new(items, workspace_weak);
        Picker::uniform_list(delegate, window, cx)
    });
}

/// Show all table and column comments from the connected database.
fn view_comments_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        c.relname AS object_name, \
        CASE c.relkind \
            WHEN 'r' THEN 'table' \
            WHEN 'v' THEN 'view' \
            WHEN 'm' THEN 'materialized view' \
            WHEN 'S' THEN 'sequence' \
        END AS object_type, \
        obj_description(c.oid) AS comment \
    FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast') \
        AND obj_description(c.oid) IS NOT NULL \
    UNION ALL \
    SELECT \
        c.relname || '.' || a.attname, \
        'column', \
        col_description(c.oid, a.attnum) \
    FROM pg_class c \
    JOIN pg_namespace n ON n.oid = c.relnamespace \
    JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast') \
        AND col_description(c.oid, a.attnum) IS NOT NULL \
    ORDER BY object_type, object_name";

    execute_system_query(workspace, sql, window, cx);
}

/// Show table privileges for non-system schemas.
fn view_privileges_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        grantee, \
        table_schema, \
        table_name, \
        privilege_type, \
        is_grantable \
    FROM information_schema.table_privileges \
    WHERE table_schema NOT IN ('pg_catalog', 'information_schema') \
    ORDER BY table_schema, table_name, grantee, privilege_type";

    execute_system_query(workspace, sql, window, cx);
}

/// Show database roles and their attributes.
fn view_roles_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "SELECT \
        rolname AS role, \
        rolsuper AS superuser, \
        rolcreaterole AS can_create_role, \
        rolcreatedb AS can_create_db, \
        rolcanlogin AS can_login, \
        rolconnlimit AS conn_limit, \
        rolvaliduntil AS valid_until \
    FROM pg_roles \
    WHERE rolname NOT LIKE 'pg_%' \
    ORDER BY rolname";

    execute_system_query(workspace, sql, window, cx);
}

/// Show tablespace information including size and location.
fn view_tablespaces_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        spcname AS tablespace, \
        pg_size_pretty(pg_tablespace_size(spcname)) AS size, \
        spcowner::regrole AS owner, \
        spclocation AS location \
    FROM pg_tablespace \
    ORDER BY spcname";

    execute_system_query(workspace, sql, window, cx);
}

/// Show replication status for connected replicas.
fn view_replication_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        pid, \
        usename AS user_name, \
        application_name AS app, \
        client_addr AS client, \
        state, \
        sent_lsn, \
        write_lsn, \
        flush_lsn, \
        replay_lsn, \
        sync_state \
    FROM pg_stat_replication \
    ORDER BY application_name";

    execute_system_query(workspace, sql, window, cx);
}

/// Generate a full DDL dump for all schemas in the schema tree.
fn dump_schema_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(schema_tree) = conn_panel.read(cx).schema_tree() else {
        tracing::warn!("DumpSchema: no schema tree available (not connected or schema not loaded)");
        return;
    };

    let mut ddl = String::new();
    ddl.push_str("-- PgBlade Schema Dump\n");
    ddl.push_str(&format!(
        "-- Generated: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));

    for schema in &schema_tree.schemas {
        ddl.push_str(&format!("-- Schema: {}\n\n", schema.info.name));

        // Sequences first
        for seq in &schema.sequences {
            ddl.push_str(&format!(
                "CREATE SEQUENCE IF NOT EXISTS \"{}\".\"{}\";\n\n",
                schema.info.name, seq.name
            ));
        }

        // Tables
        for table in &schema.tables {
            ddl.push_str(&ConnectionPanel::generate_ddl(table));
            ddl.push('\n');

            // Indexes (already included in generate_ddl, but add schema-qualified IF NOT EXISTS)
            for idx in &table.indexes {
                let unique = if idx.is_unique { "UNIQUE " } else { "" };
                let cols = idx
                    .columns
                    .iter()
                    .map(|c| format!("\"{}\"", c))
                    .collect::<Vec<_>>()
                    .join(", ");
                ddl.push_str(&format!(
                    "CREATE {unique}INDEX IF NOT EXISTS \"{}\" ON \"{}\".\"{}\" USING {} ({cols});\n",
                    idx.name, schema.info.name, table.info.name, idx.index_type
                ));
            }
            ddl.push('\n');
        }

        // Views
        for view in &schema.views {
            ddl.push_str(&format!(
                "-- View: {}.{}\n",
                schema.info.name, view.info.name
            ));
            ddl.push_str("-- (View definition not available from introspection)\n\n");
        }
    }

    // Show in result panel
    workspace.open_panel::<ResultPanel>(window, cx);
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(ddl, cx);
        });
    }
}

/// Show the SQL definition of a view or materialized view.
/// Reads the view name from the selected text in the active editor.
fn view_definition_action(
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
        return;
    };
    let view_name = selected.trim();
    let safe_name = view_name.replace('\'', "''");

    let sql = format!(
        "SELECT definition FROM pg_views WHERE viewname = '{safe_name}' \
         UNION ALL \
         SELECT definition FROM pg_matviews WHERE matviewname = '{safe_name}'"
    );
    execute_system_query(workspace, &sql, window, cx);
}

/// Show column-level statistics from pg_stats for a table.
/// Reads the table name from the selected text in the active editor.
fn column_stats_action(
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
        return;
    };
    let table_name = selected.trim().replace('\'', "''");

    let sql = format!(
        "SELECT \
            attname AS column, \
            n_distinct, \
            most_common_vals::text AS common_values, \
            most_common_freqs::text AS common_frequencies, \
            null_frac AS null_fraction, \
            avg_width \
        FROM pg_stats \
        WHERE tablename = '{table_name}' \
        ORDER BY attname"
    );
    execute_system_query(workspace, &sql, window, cx);
}

/// Show the progress of any currently running VACUUM operations.
fn vacuum_progress_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        p.pid, \
        a.datname AS database, \
        t.relname AS table_name, \
        p.phase, \
        p.heap_blks_total, \
        p.heap_blks_scanned, \
        p.heap_blks_vacuumed, \
        CASE WHEN p.heap_blks_total > 0 \
            THEN round(100.0 * p.heap_blks_vacuumed / p.heap_blks_total, 1) \
            ELSE 0 \
        END AS progress_pct \
    FROM pg_stat_progress_vacuum p \
    JOIN pg_stat_activity a ON a.pid = p.pid \
    JOIN pg_class t ON t.oid = p.relid \
    ORDER BY p.pid";

    execute_system_query(workspace, sql, window, cx);
}

/// Show cache hit ratios for indexes, tables, and the overall database.
fn cache_hit_ratio_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        'index hit rate' AS metric, \
        CASE WHEN (sum(idx_blks_hit) + sum(idx_blks_read)) > 0 \
            THEN round(sum(idx_blks_hit) / (sum(idx_blks_hit) + sum(idx_blks_read))::numeric, 4) \
            ELSE 0 END AS ratio \
    FROM pg_statio_user_indexes \
    UNION ALL \
    SELECT \
        'table hit rate', \
        CASE WHEN (sum(heap_blks_hit) + sum(heap_blks_read)) > 0 \
            THEN round(sum(heap_blks_hit) / (sum(heap_blks_hit) + sum(heap_blks_read))::numeric, 4) \
            ELSE 0 END \
    FROM pg_statio_user_tables \
    UNION ALL \
    SELECT \
        'total hit rate', \
        CASE WHEN (sum(blks_hit) + sum(blks_read)) > 0 \
            THEN round(sum(blks_hit) / (sum(blks_hit) + sum(blks_read))::numeric, 4) \
            ELSE 0 END \
    FROM pg_stat_database \
    WHERE datname = current_database()";

    execute_system_query(workspace, sql, window, cx);
}

/// Show non-default and important PostgreSQL settings.
fn view_settings_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT name, setting, unit, short_desc, source, boot_val \
    FROM pg_settings \
    WHERE source != 'default' OR name IN ('max_connections', 'shared_buffers', 'work_mem', \
        'maintenance_work_mem', 'effective_cache_size', 'random_page_cost', 'wal_level', \
        'max_wal_size', 'checkpoint_completion_target', 'log_min_duration_statement', \
        'autovacuum', 'timezone') \
    ORDER BY name";

    execute_system_query(workspace, sql, window, cx);
}

/// Show sequence values for user schemas.
fn sequence_values_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT schemaname AS schema, sequencename AS sequence, \
        data_type, start_value, min_value, max_value, increment_by, \
        last_value, cycle AS is_cyclic, cache_size \
    FROM pg_sequences \
    WHERE schemaname NOT IN ('pg_catalog', 'information_schema') \
    ORDER BY schemaname, sequencename";

    execute_system_query(workspace, sql, window, cx);
}

/// Show table dependencies (what depends on the selected table).
/// Reads the table name from the selected text in the active editor.
fn view_dependencies_action(
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
        return;
    };
    let table_name = selected.trim().replace('\'', "''");

    let sql = format!(
        "SELECT DISTINCT \
            d.classid::regclass AS source_type, \
            d.objid::regclass AS dependent_object, \
            d.deptype AS dependency_type \
        FROM pg_depend d \
        JOIN pg_class c ON c.oid = d.refobjid \
        WHERE c.relname = '{table_name}' \
            AND d.deptype IN ('n', 'a') \
            AND d.classid = 'pg_class'::regclass \
        UNION \
        SELECT \
            'foreign_key' AS source_type, \
            conrelid::regclass AS dependent_object, \
            'fk: ' || conname AS dependency_type \
        FROM pg_constraint \
        WHERE confrelid = (SELECT oid FROM pg_class WHERE relname = '{table_name}') \
        ORDER BY dependent_object"
    );
    execute_system_query(workspace, &sql, window, cx);
}

/// Show database connection statistics grouped by database.
fn connection_stats_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        datname AS database, \
        count(*) AS total, \
        count(*) FILTER (WHERE state = 'active') AS active, \
        count(*) FILTER (WHERE state = 'idle') AS idle, \
        count(*) FILTER (WHERE state = 'idle in transaction') AS idle_in_tx, \
        count(*) FILTER (WHERE wait_event_type IS NOT NULL) AS waiting, \
        max(now() - query_start) FILTER (WHERE state = 'active') AS longest_query \
    FROM pg_stat_activity \
    WHERE datname IS NOT NULL \
    GROUP BY datname \
    ORDER BY total DESC";

    execute_system_query(workspace, sql, window, cx);
}

/// Show WAL (Write-Ahead Log) status and configuration.
fn wal_status_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "SELECT \
        pg_walfile_name(pg_current_wal_lsn()) AS current_wal_file, \
        pg_current_wal_lsn() AS current_lsn, \
        pg_size_pretty(pg_wal_lsn_diff(pg_current_wal_lsn(), '0/0')) AS total_wal_generated, \
        (SELECT count(*) FROM pg_ls_waldir()) AS wal_files_count, \
        pg_size_pretty((SELECT sum(size) FROM pg_ls_waldir())) AS wal_directory_size, \
        (SELECT setting FROM pg_settings WHERE name = 'wal_level') AS wal_level, \
        (SELECT setting FROM pg_settings WHERE name = 'max_wal_size') AS max_wal_size";

    execute_system_query(workspace, sql, window, cx);
}

/// Show tables with the most dead tuples (bloat candidates).
fn table_bloat_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "SELECT \
        schemaname || '.' || tablename AS table_name, \
        pg_size_pretty(pg_total_relation_size(schemaname || '.' || tablename)) AS total_size, \
        CASE WHEN n_live_tup > 0 THEN \
            round(100.0 * n_dead_tup / (n_live_tup + n_dead_tup), 1) \
        ELSE 0 END AS dead_tuple_pct, \
        n_live_tup AS live_tuples, \
        n_dead_tup AS dead_tuples, \
        last_vacuum::text, \
        last_autovacuum::text, \
        last_analyze::text \
    FROM pg_stat_user_tables \
    WHERE n_dead_tup > 0 \
    ORDER BY n_dead_tup DESC \
    LIMIT 30";

    execute_system_query(workspace, sql, window, cx);
}

/// Show indexes that have never been used (candidates for removal).
fn unused_indexes_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        schemaname || '.' || relname AS table_name, \
        indexrelname AS index_name, \
        pg_size_pretty(pg_relation_size(indexrelid)) AS index_size, \
        idx_scan AS times_used, \
        idx_tup_read AS tuples_read \
    FROM pg_stat_user_indexes \
    WHERE idx_scan = 0 \
        AND indexrelname NOT LIKE '%_pkey' \
    ORDER BY pg_relation_size(indexrelid) DESC \
    LIMIT 30";

    execute_system_query(workspace, sql, window, cx);
}

/// Show tables with high sequential scan ratios (missing index candidates).
fn missing_indexes_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        schemaname || '.' || relname AS table_name, \
        seq_scan AS sequential_scans, \
        idx_scan AS index_scans, \
        CASE WHEN (seq_scan + idx_scan) > 0 THEN \
            round(100.0 * seq_scan / (seq_scan + idx_scan), 1) \
        ELSE 0 END AS seq_scan_pct, \
        n_live_tup AS estimated_rows, \
        pg_size_pretty(pg_relation_size(relid)) AS table_size \
    FROM pg_stat_user_tables \
    WHERE seq_scan > idx_scan \
        AND n_live_tup > 1000 \
    ORDER BY seq_scan DESC \
    LIMIT 20";

    execute_system_query(workspace, sql, window, cx);
}

/// Show table partitions with their bounds, sizes, and estimated rows.
fn view_partitions_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        parent.relname AS parent_table, \
        child.relname AS partition, \
        pg_get_expr(child.relpartbound, child.oid) AS partition_bound, \
        pg_size_pretty(pg_relation_size(child.oid)) AS size, \
        (SELECT reltuples::bigint FROM pg_class WHERE oid = child.oid) AS est_rows \
    FROM pg_inherits \
    JOIN pg_class parent ON parent.oid = inhparent \
    JOIN pg_class child ON child.oid = inhrelid \
    JOIN pg_namespace n ON n.oid = parent.relnamespace \
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') \
    ORDER BY parent.relname, child.relname";

    execute_system_query(workspace, sql, window, cx);
}

/// Show active autovacuum workers and their progress.
fn autovacuum_status_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        pid, \
        datname AS database, \
        relid::regclass AS table_name, \
        phase, \
        heap_blks_total, \
        heap_blks_scanned, \
        CASE WHEN heap_blks_total > 0 THEN \
            round(100.0 * heap_blks_scanned / heap_blks_total, 1) \
        ELSE 0 END AS scan_pct, \
        now() - query_start AS duration \
    FROM pg_stat_progress_vacuum \
    JOIN pg_stat_activity USING (pid) \
    ORDER BY query_start";

    execute_system_query(workspace, sql, window, cx);
}

/// Show foreign tables and their associated foreign servers.
fn view_foreign_tables_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        foreign_table_schema AS schema, \
        foreign_table_name AS table_name, \
        foreign_server_name AS server \
    FROM information_schema.foreign_tables \
    ORDER BY foreign_table_schema, foreign_table_name";

    execute_system_query(workspace, sql, window, cx);
}

/// Insert a CREATE FUNCTION template into the active editor.
fn create_function_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let template = "CREATE OR REPLACE FUNCTION my_function(param1 integer, param2 text)\n\
        RETURNS void AS $$\n\
        BEGIN\n\
            -- Your code here\n\
            RAISE NOTICE 'Hello from my_function';\n\
        END;\n\
        $$ LANGUAGE plpgsql;";

    if let Some(active_item) = workspace.active_item(cx)
        && let Some(editor) = active_item.act_as::<Editor>(cx)
    {
        editor.update(cx, |editor, cx| {
            let text = editor.text(cx);
            let prefix = if text.is_empty() { "" } else { "\n\n" };
            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
            editor.insert(&format!("{prefix}{template}"), window, cx);
        });
    }
}

/// Insert a CREATE TRIGGER template into the active editor.
fn create_trigger_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let template = "CREATE OR REPLACE FUNCTION my_trigger_function()\n\
        RETURNS trigger AS $$\n\
        BEGIN\n\
            -- Your trigger logic here\n\
            RETURN NEW;\n\
        END;\n\
        $$ LANGUAGE plpgsql;\n\n\
        CREATE TRIGGER my_trigger\n\
            BEFORE INSERT OR UPDATE ON my_table\n\
            FOR EACH ROW\n\
            EXECUTE FUNCTION my_trigger_function();";

    if let Some(active_item) = workspace.active_item(cx)
        && let Some(editor) = active_item.act_as::<Editor>(cx)
    {
        editor.update(cx, |editor, cx| {
            let text = editor.text(cx);
            let prefix = if text.is_empty() { "" } else { "\n\n" };
            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
            editor.insert(&format!("{prefix}{template}"), window, cx);
        });
    }
}
