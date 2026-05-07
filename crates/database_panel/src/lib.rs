mod connection_panel;
mod database_status;
mod result_panel;
mod sql_completion;
pub mod sql_completion_provider;

use std::rc::Rc;
use std::sync::Arc;

use editor::Editor;
use editor::actions::SelectAll;
use gpui::{App, AppContext as _, ClipboardItem, Context, Entity, Window, actions};
use picker::Picker;
use tokio::runtime::Runtime;
use workspace::Workspace;

use pgblade_core::connection::{ConnectionId, ConnectionProfile, Environment, SshConfig, SslMode};
use pgblade_core::driver::DatabaseSession;
use result_panel::ResultPanelEvent;

pub use connection_panel::ConnectionPanel;
pub use database_status::DatabaseStatusItem;
pub use result_panel::ResultPanel;
pub use sql_completion_provider::SqlCompletionProvider;

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
        ImportCsvExecute,
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
        CreateTrigger,
        ShowShortcuts,
        StatStatements,
        TableAccessStats,
        ActiveQueries,
        ConnectionLimits,
        MaintenanceRecommendations,
        DuplicateIndexes,
        SaveBookmark,
        ViewBookmarks,
        SnippetCte,
        SnippetWindowFunction,
        SnippetUpsert,
        SnippetPivot,
        SnippetRecursiveCte,
        SnippetJsonQuery,
        SnippetLateralJoin,
        SnippetSelectJoin,
        SnippetGroupSummary,
        TableStructure,
        GenerateAlterTable,
        HealthCheck,
        CopyQualifiedName,
        GenerateGrant,
        AdminCommands,
        DetectEnvDatabase,
        SetTimeout,
        RefreshSchema,
        LintSql,
        CompareExplain,
        ExplainQueryAI
    ]
);

pub fn init(cx: &mut App) {
    let provider = Rc::new(SqlCompletionProvider::new());

    // Observe new editors and install the SQL completion provider as a wrapper
    // around any existing provider (typically the LSP-based Project provider).
    let provider_for_editors = provider.clone();
    cx.observe_new(
        move |editor: &mut Editor, _window: Option<&mut Window>, _cx: &mut Context<Editor>| {
            let existing = editor.completion_provider();
            let wrapper = provider_for_editors.clone();
            wrapper.set_inner(existing);
            editor.set_completion_provider(Some(wrapper as Rc<dyn editor::CompletionProvider>));
        },
    )
    .detach();

    let provider_for_workspace = provider;
    cx.observe_new(
        move |workspace: &mut Workspace,
              window: Option<&mut Window>,
              cx: &mut Context<Workspace>| {
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

            // Register the ShowShortcuts action on the workspace
            workspace.register_action(|workspace, _: &ShowShortcuts, window, cx| {
                show_shortcuts_action(workspace, window, cx);
            });

            // Register the StatStatements action on the workspace
            workspace.register_action(|workspace, _: &StatStatements, window, cx| {
                stat_statements_action(workspace, window, cx);
            });

            // Register the TableAccessStats action on the workspace
            workspace.register_action(|workspace, _: &TableAccessStats, window, cx| {
                table_access_stats_action(workspace, window, cx);
            });

            // Register the ActiveQueries action on the workspace
            workspace.register_action(|workspace, _: &ActiveQueries, window, cx| {
                active_queries_action(workspace, window, cx);
            });

            // Register the ConnectionLimits action on the workspace
            workspace.register_action(|workspace, _: &ConnectionLimits, window, cx| {
                connection_limits_action(workspace, window, cx);
            });

            // Register the MaintenanceRecommendations action on the workspace
            workspace.register_action(|workspace, _: &MaintenanceRecommendations, window, cx| {
                maintenance_recommendations_action(workspace, window, cx);
            });

            // Register the DuplicateIndexes action on the workspace
            workspace.register_action(|workspace, _: &DuplicateIndexes, window, cx| {
                duplicate_indexes_action(workspace, window, cx);
            });

            // Register the SaveBookmark action on the workspace
            workspace.register_action(|workspace, _: &SaveBookmark, _window, cx| {
                save_bookmark_action(workspace, cx);
            });

            // Register the ViewBookmarks action on the workspace
            workspace.register_action(|workspace, _: &ViewBookmarks, window, cx| {
                view_bookmarks_action(workspace, window, cx);
            });

            // Register SQL snippet actions on the workspace
            workspace.register_action(|workspace, _: &SnippetCte, window, cx| {
                snippet_cte_action(workspace, window, cx);
            });

            workspace.register_action(|workspace, _: &SnippetWindowFunction, window, cx| {
                snippet_window_function_action(workspace, window, cx);
            });

            workspace.register_action(|workspace, _: &SnippetUpsert, window, cx| {
                snippet_upsert_action(workspace, window, cx);
            });

            workspace.register_action(|workspace, _: &SnippetPivot, window, cx| {
                snippet_pivot_action(workspace, window, cx);
            });

            workspace.register_action(|workspace, _: &SnippetRecursiveCte, window, cx| {
                snippet_recursive_cte_action(workspace, window, cx);
            });

            workspace.register_action(|workspace, _: &SnippetJsonQuery, window, cx| {
                snippet_json_query_action(workspace, window, cx);
            });

            workspace.register_action(|workspace, _: &SnippetLateralJoin, window, cx| {
                snippet_lateral_join_action(workspace, window, cx);
            });

            // Register the TableStructure action on the workspace
            workspace.register_action(|workspace, _: &TableStructure, window, cx| {
                table_structure_action(workspace, window, cx);
            });

            // Register the GenerateAlterTable action on the workspace
            workspace.register_action(|workspace, _: &GenerateAlterTable, window, cx| {
                generate_alter_table_action(workspace, window, cx);
            });

            // Register the ImportCsvExecute action on the workspace
            workspace.register_action(|workspace, _: &ImportCsvExecute, window, cx| {
                import_csv_execute_action(workspace, window, cx);
            });

            // Register the SnippetSelectJoin action on the workspace
            workspace.register_action(|workspace, _: &SnippetSelectJoin, window, cx| {
                snippet_select_join_action(workspace, window, cx);
            });

            // Register the SnippetGroupSummary action on the workspace
            workspace.register_action(|workspace, _: &SnippetGroupSummary, window, cx| {
                snippet_group_summary_action(workspace, window, cx);
            });

            // Register the HealthCheck action on the workspace
            workspace.register_action(|workspace, _: &HealthCheck, window, cx| {
                health_check_action(workspace, window, cx);
            });

            // Register the CopyQualifiedName action on the workspace
            workspace.register_action(|workspace, _: &CopyQualifiedName, _window, cx| {
                copy_qualified_name_action(workspace, cx);
            });

            // Register the GenerateGrant action on the workspace
            workspace.register_action(|workspace, _: &GenerateGrant, window, cx| {
                generate_grant_action(workspace, window, cx);
            });

            // Register the AdminCommands action on the workspace
            workspace.register_action(|workspace, _: &AdminCommands, window, cx| {
                admin_commands_action(workspace, window, cx);
            });

            // Register the DetectEnvDatabase action on the workspace
            workspace.register_action(|workspace, _: &DetectEnvDatabase, window, cx| {
                detect_env_database_action(workspace, window, cx);
            });

            // Register the SetTimeout action on the workspace
            workspace.register_action(|workspace, _: &SetTimeout, window, cx| {
                set_timeout_action(workspace, window, cx);
            });

            // Register the RefreshSchema action on the workspace
            workspace.register_action(|workspace, _: &RefreshSchema, _window, cx| {
                if let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) {
                    conn_panel.update(cx, |panel, cx| {
                        panel.fetch_schema(cx);
                    });
                }
            });

            // Register the LintSql action on the workspace
            workspace.register_action(|workspace, _: &LintSql, window, cx| {
                lint_sql_action(workspace, window, cx);
            });

            // Register the CompareExplain action on the workspace
            workspace.register_action(|workspace, _: &CompareExplain, window, cx| {
                compare_explain_action(workspace, window, cx);
            });

            // Register the ExplainQueryAI action on the workspace
            workspace.register_action(|workspace, _: &ExplainQueryAI, window, cx| {
                explain_query_ai_action(workspace, window, cx);
            });

            if let Some(window) = window {
                let workspace_weak = cx.weak_entity();
                let provider_clone = provider_for_workspace.clone();
                let connection =
                    cx.new(|cx| ConnectionPanel::new(workspace_weak, provider_clone, window, cx));
                workspace.add_panel(connection.clone(), window, cx);

                let results = cx.new(ResultPanel::new);

                // Subscribe to ResultPanel events for schema change detection (Feature 4)
                cx.subscribe(
                    &results,
                    |workspace: &mut Workspace, _panel, event: &ResultPanelEvent, cx| match event {
                        ResultPanelEvent::SchemaChanged => {
                            if let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) {
                                conn_panel.update(cx, |panel, cx| {
                                    panel.fetch_schema(cx);
                                });
                            }
                        }
                    },
                )
                .detach();

                workspace.add_panel(results, window, cx);

                // Auto-open the connection panel if there are saved connections
                let has_saved = connection.read(cx).has_saved_connections();
                if has_saved {
                    workspace.open_panel::<ConnectionPanel>(window, cx);
                }
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

    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Strip common string delimiters from code files
    let cleaned = strip_string_delimiters(trimmed);

    Some(cleaned)
}

/// Strip common string delimiters so SQL selected inside Python, JS, Rust, Go,
/// etc. source files can be executed directly.
fn strip_string_delimiters(s: &str) -> String {
    let trimmed = s.trim();

    // Triple-quoted strings (Python, etc.)
    if trimmed.starts_with("\"\"\"") && trimmed.ends_with("\"\"\"") && trimmed.len() >= 6 {
        return trimmed[3..trimmed.len() - 3].trim().to_string();
    }
    if trimmed.starts_with("'''") && trimmed.ends_with("'''") && trimmed.len() >= 6 {
        return trimmed[3..trimmed.len() - 3].trim().to_string();
    }

    // Rust raw strings: r##"..."## (check before r#"..."#)
    if trimmed.starts_with("r##\"") && trimmed.ends_with("\"##") && trimmed.len() >= 7 {
        return trimmed[4..trimmed.len() - 3].trim().to_string();
    }
    if trimmed.starts_with("r#\"") && trimmed.ends_with("\"#") && trimmed.len() >= 5 {
        return trimmed[3..trimmed.len() - 2].trim().to_string();
    }

    // Template literals / Go raw strings (backticks)
    if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() >= 2 {
        return trimmed[1..trimmed.len() - 1].trim().to_string();
    }

    // Regular double or single quotes
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return trimmed[1..trimmed.len() - 1].trim().to_string();
    }

    // Strip common string concatenation artifacts
    trimmed
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\" +", "")
        .replace("+ \"", "")
        .replace("\" &", "")
        .replace("& \"", "")
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
        // Show "not connected" message in result panel
        workspace.open_panel::<ResultPanel>(window, cx);
        if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
            result_panel.update(cx, |panel, cx| {
                panel.show_ddl(
                    "Not connected to a database.\n\n\
                     Click the database icon in the sidebar to connect,\n\
                     or press Cmd+Shift+P and type 'Show Shortcuts' for help."
                        .to_string(),
                    cx,
                );
            });
        }
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
        // Show "not connected" message in result panel
        workspace.open_panel::<ResultPanel>(window, cx);
        if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
            result_panel.update(cx, |panel, cx| {
                panel.show_ddl(
                    "Not connected to a database.\n\n\
                     Click the database icon in the sidebar to connect,\n\
                     or press Cmd+Shift+P and type 'Show Shortcuts' for help."
                        .to_string(),
                    cx,
                );
            });
        }
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
    let safe_term = connection_panel::escape_sql_like(term);

    let sql = format!(
        "SELECT 'TABLE' as type, table_schema as schema, table_name as name, '' as detail \
         FROM information_schema.tables \
         WHERE table_name ILIKE '%{safe_term}%' ESCAPE '\\' AND table_schema NOT IN ('pg_catalog', 'information_schema') \
         UNION ALL \
         SELECT 'COLUMN', table_schema, table_name || '.' || column_name, data_type \
         FROM information_schema.columns \
         WHERE column_name ILIKE '%{safe_term}%' ESCAPE '\\' AND table_schema NOT IN ('pg_catalog', 'information_schema') \
         UNION ALL \
         SELECT 'FUNCTION', n.nspname, p.proname, pg_get_function_result(p.oid) \
         FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
         WHERE p.proname ILIKE '%{safe_term}%' ESCAPE '\\' AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
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
    let safe_name = connection_panel::escape_sql_string(view_name);

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
    let table_name = connection_panel::escape_sql_string(selected.trim());

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
    let table_name = connection_panel::escape_sql_string(selected.trim());

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

/// Display all PgBlade keyboard shortcuts in the result panel.
fn show_shortcuts_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let shortcuts = "\
PgBlade Keyboard Shortcuts
==========================

Query Execution:
  Cmd+Enter          Execute query (selected text or full buffer)
  Cmd+Alt+X          EXPLAIN ANALYZE
  Ctrl+Space         SQL auto-completion

Navigation:
  Ctrl+Shift+D       Toggle Database Panel
  Cmd+Shift+P        Command palette (type any action name)

All actions are available in the command palette:
  Execute Query, Format SQL, View Sessions, Slow Queries,
  View Locks, Kill Backend, Database Info, Table Sizes,
  Index Usage, Unused Indexes, Missing Indexes, Table Bloat,
  Cache Hit Ratio, Vacuum Progress, Autovacuum Status,
  Connection Stats, WAL Status, View Privileges, View Roles,
  View Tablespaces, View Replication, View Settings,
  View Comments, View Extensions, View Definition,
  View Dependencies, View Partitions, View Foreign Tables,
  Sequence Values, Column Stats, Compare Schemas,
  Dump Schema, ER Diagram, Import CSV, View History,
  Search Objects, Disconnect, Create Function, Create Trigger,
  Stat Statements, Table Access Stats, Active Queries,
  Connection Limits, Maintenance Recommendations,
  Duplicate Indexes, Show Shortcuts, Lint SQL, Compare Explain,
  Explain Query AI";

    workspace.open_panel::<ResultPanel>(window, cx);
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(shortcuts.to_string(), cx);
        });
    }
}

/// Show the top queries by total execution time from pg_stat_statements.
/// Requires the pg_stat_statements extension to be installed.
fn stat_statements_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        calls, \
        round(total_exec_time::numeric, 2) AS total_time_ms, \
        round(mean_exec_time::numeric, 2) AS avg_time_ms, \
        round(max_exec_time::numeric, 2) AS max_time_ms, \
        rows, \
        shared_blks_hit + shared_blks_read AS total_blks, \
        CASE WHEN shared_blks_hit + shared_blks_read > 0 \
            THEN round(100.0 * shared_blks_hit / (shared_blks_hit + shared_blks_read), 1) \
            ELSE 0 END AS cache_hit_pct, \
        LEFT(query, 300) AS query \
    FROM pg_stat_statements \
    WHERE userid = (SELECT usesysid FROM pg_user WHERE usename = current_user) \
    ORDER BY total_exec_time DESC \
    LIMIT 25";

    execute_system_query(workspace, sql, window, cx);
}

/// Show table access statistics ordered by total scans.
fn table_access_stats_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        schemaname || '.' || relname AS table_name, \
        seq_scan, \
        seq_tup_read, \
        idx_scan, \
        idx_tup_fetch, \
        n_tup_ins AS inserts, \
        n_tup_upd AS updates, \
        n_tup_del AS deletes, \
        n_tup_hot_upd AS hot_updates, \
        n_live_tup AS live_rows, \
        n_dead_tup AS dead_rows \
    FROM pg_stat_user_tables \
    ORDER BY seq_scan + COALESCE(idx_scan, 0) DESC \
    LIMIT 30";

    execute_system_query(workspace, sql, window, cx);
}

/// Show currently active queries (excluding this session).
fn active_queries_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        pid, \
        usename AS user_name, \
        datname AS database, \
        state, \
        now() - query_start AS duration, \
        wait_event_type, \
        wait_event, \
        LEFT(query, 500) AS query \
    FROM pg_stat_activity \
    WHERE state = 'active' \
        AND pid != pg_backend_pid() \
    ORDER BY query_start \
    LIMIT 50";

    execute_system_query(workspace, sql, window, cx);
}

/// Show database connection limits per role with current usage.
fn connection_limits_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        rolname AS role, \
        rolconnlimit AS max_connections, \
        (SELECT count(*) FROM pg_stat_activity WHERE usename = rolname) AS current_connections, \
        CASE WHEN rolconnlimit > 0 \
            THEN round(100.0 * (SELECT count(*) FROM pg_stat_activity WHERE usename = rolname) / rolconnlimit, 1) \
            ELSE 0 END AS usage_pct \
    FROM pg_roles \
    WHERE rolcanlogin = true \
    ORDER BY current_connections DESC";

    execute_system_query(workspace, sql, window, cx);
}

/// Show tables that need maintenance (vacuum, analyze, reindex).
fn maintenance_recommendations_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        schemaname || '.' || relname AS table_name, \
        n_live_tup AS live_rows, \
        n_dead_tup AS dead_rows, \
        CASE WHEN n_live_tup > 0 THEN round(100.0 * n_dead_tup / n_live_tup, 1) ELSE 0 END AS bloat_pct, \
        last_vacuum::date AS last_vacuum, \
        last_autovacuum::date AS last_autovacuum, \
        last_analyze::date AS last_analyze, \
        last_autoanalyze::date AS last_autoanalyze, \
        CASE \
            WHEN n_dead_tup > n_live_tup * 0.2 THEN 'VACUUM NEEDED' \
            WHEN last_analyze IS NULL AND last_autoanalyze IS NULL THEN 'ANALYZE NEEDED' \
            WHEN last_vacuum IS NULL AND last_autovacuum IS NULL AND n_dead_tup > 1000 THEN 'VACUUM RECOMMENDED' \
            ELSE 'OK' \
        END AS recommendation \
    FROM pg_stat_user_tables \
    WHERE n_dead_tup > 100 OR last_analyze IS NULL \
    ORDER BY n_dead_tup DESC \
    LIMIT 30";

    execute_system_query(workspace, sql, window, cx);
}

/// Find duplicate indexes that waste disk space.
fn duplicate_indexes_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT \
        pg_size_pretty(sum(pg_relation_size(idx))::bigint) AS total_size, \
        array_agg(idx) AS indexes, \
        (array_agg(idx))[1] AS table_name, \
        array_agg(pg_get_indexdef(idx)) AS definitions \
    FROM ( \
        SELECT indexrelid::regclass AS idx, \
            (indrelid::text || E'\\n' || indclass::text || E'\\n' || \
             indkey::text || E'\\n' || coalesce(indexprs::text, '') || \
             E'\\n' || coalesce(indpred::text, '')) AS key \
        FROM pg_index \
    ) sub \
    GROUP BY key \
    HAVING count(*) > 1 \
    ORDER BY sum(pg_relation_size(idx)) DESC";

    execute_system_query(workspace, sql, window, cx);
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

/// Save the current editor SQL as a bookmark.
fn save_bookmark_action(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(sql) = get_sql_from_editor(&editor, cx) else {
        return;
    };

    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    conn_panel.update(cx, |panel, _cx| {
        panel.save_bookmark(&sql);
        tracing::info!("Bookmark saved");
    });
}

/// Display all saved bookmarks in the result panel.
fn view_bookmarks_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let bookmarks = conn_panel.read(cx).load_bookmarks();

    let columns = vec![
        pgblade_core::result::ColumnMeta {
            name: "Name".to_string(),
            type_name: "text".to_string(),
            nullable: false,
        },
        pgblade_core::result::ColumnMeta {
            name: "SQL".to_string(),
            type_name: "text".to_string(),
            nullable: false,
        },
        pgblade_core::result::ColumnMeta {
            name: "Created".to_string(),
            type_name: "text".to_string(),
            nullable: false,
        },
    ];
    let rows: Vec<Vec<pgblade_core::result::CellValue>> = bookmarks
        .iter()
        .map(|(_id, name, sql, created)| {
            vec![
                pgblade_core::result::CellValue::Text(name.clone()),
                pgblade_core::result::CellValue::Text(sql.chars().take(200).collect()),
                pgblade_core::result::CellValue::Text(created.clone()),
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

/// Shared helper to insert SQL text into the active editor.
fn insert_into_editor(
    workspace: &mut Workspace,
    sql: &str,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if let Some(active_item) = workspace.active_item(cx)
        && let Some(editor) = active_item.act_as::<Editor>(cx)
    {
        editor.update(cx, |editor, cx| {
            let prefix = if editor.text(cx).is_empty() {
                ""
            } else {
                "\n\n"
            };
            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
            editor.insert(&format!("{prefix}{sql}"), window, cx);
        });
    }
}

/// Insert a CTE snippet into the active editor.
fn snippet_cte_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let sql = "WITH cte AS (\n    SELECT column1, column2\n    FROM table_name\n    WHERE condition\n)\nSELECT * FROM cte;";
    insert_into_editor(workspace, sql, window, cx);
}

/// Insert a window function snippet into the active editor.
fn snippet_window_function_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT\n    column1,\n    ROW_NUMBER() OVER (PARTITION BY column2 ORDER BY column3 DESC) AS row_num,\n    SUM(column4) OVER (PARTITION BY column2) AS total\nFROM table_name;";
    insert_into_editor(workspace, sql, window, cx);
}

/// Insert an UPSERT snippet into the active editor.
fn snippet_upsert_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "INSERT INTO table_name (column1, column2)\nVALUES ($1, $2)\nON CONFLICT (column1)\nDO UPDATE SET column2 = EXCLUDED.column2\nRETURNING *;";
    insert_into_editor(workspace, sql, window, cx);
}

/// Insert a pivot/FILTER snippet into the active editor.
fn snippet_pivot_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT\n    category,\n    COUNT(*) FILTER (WHERE status = 'active') AS active,\n    COUNT(*) FILTER (WHERE status = 'inactive') AS inactive,\n    COUNT(*) AS total\nFROM table_name\nGROUP BY category\nORDER BY total DESC;";
    insert_into_editor(workspace, sql, window, cx);
}

/// Insert a recursive CTE snippet into the active editor.
fn snippet_recursive_cte_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "WITH RECURSIVE tree AS (\n    -- Base case\n    SELECT id, parent_id, name, 1 AS depth\n    FROM categories\n    WHERE parent_id IS NULL\n    \n    UNION ALL\n    \n    -- Recursive case\n    SELECT c.id, c.parent_id, c.name, t.depth + 1\n    FROM categories c\n    JOIN tree t ON c.parent_id = t.id\n)\nSELECT * FROM tree ORDER BY depth, name;";
    insert_into_editor(workspace, sql, window, cx);
}

/// Insert a JSONB query snippet into the active editor.
fn snippet_json_query_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT\n    id,\n    data->>'name' AS name,\n    data->'address'->>'city' AS city,\n    jsonb_array_length(data->'tags') AS tag_count\nFROM documents\nWHERE data @> '{\"type\": \"user\"}'\nORDER BY data->>'name';";
    insert_into_editor(workspace, sql, window, cx);
}

/// Insert a LATERAL JOIN snippet into the active editor.
fn snippet_lateral_join_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "SELECT u.id, u.name, recent.*\nFROM users u\nCROSS JOIN LATERAL (\n    SELECT *\n    FROM orders o\n    WHERE o.user_id = u.id\n    ORDER BY o.created_at DESC\n    LIMIT 3\n) recent\nORDER BY u.name;";
    insert_into_editor(workspace, sql, window, cx);
}

/// Show the detailed structure of a table (columns, types, constraints).
/// Reads the table name from the selected text in the active editor.
fn table_structure_action(
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
    let table_name = connection_panel::escape_sql_string(selected.trim());

    let sql = format!(
        "SELECT \
            c.column_name, \
            c.data_type, \
            c.character_maximum_length, \
            c.is_nullable, \
            c.column_default, \
            col_description((c.table_schema || '.' || c.table_name)::regclass, c.ordinal_position) AS comment, \
            CASE WHEN pk.column_name IS NOT NULL THEN 'YES' ELSE 'NO' END AS is_primary_key \
        FROM information_schema.columns c \
        LEFT JOIN ( \
            SELECT kcu.column_name \
            FROM information_schema.table_constraints tc \
            JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name \
            WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_name = '{table_name}' \
        ) pk ON pk.column_name = c.column_name \
        WHERE c.table_name = '{table_name}' \
        ORDER BY c.ordinal_position"
    );
    execute_system_query(workspace, &sql, window, cx);
}

/// Generate ALTER TABLE templates for the selected table name.
/// Reads the table name from the selected text in the active editor.
fn generate_alter_table_action(
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
    let table_name = selected.trim();

    let sql = format!(
        "-- Add column\n\
         ALTER TABLE {table_name} ADD COLUMN new_column VARCHAR(255);\n\n\
         -- Modify column type\n\
         ALTER TABLE {table_name} ALTER COLUMN column_name TYPE INTEGER USING column_name::integer;\n\n\
         -- Set/drop NOT NULL\n\
         ALTER TABLE {table_name} ALTER COLUMN column_name SET NOT NULL;\n\
         ALTER TABLE {table_name} ALTER COLUMN column_name DROP NOT NULL;\n\n\
         -- Set default\n\
         ALTER TABLE {table_name} ALTER COLUMN column_name SET DEFAULT 'value';\n\n\
         -- Drop column\n\
         ALTER TABLE {table_name} DROP COLUMN column_name;\n\n\
         -- Rename column\n\
         ALTER TABLE {table_name} RENAME COLUMN old_name TO new_name;\n\n\
         -- Add constraint\n\
         ALTER TABLE {table_name} ADD CONSTRAINT constraint_name UNIQUE (column_name);\n\n\
         -- Add foreign key\n\
         ALTER TABLE {table_name} ADD CONSTRAINT fk_name FOREIGN KEY (column_name) REFERENCES other_table(id);"
    );

    insert_into_editor(workspace, &sql, window, cx);
}

/// Import CSV data from the clipboard and execute the INSERT statements directly.
/// Reads the target table name from the editor selection.
fn import_csv_execute_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(clipboard) = cx.read_from_clipboard() else {
        tracing::warn!("ImportCsvExecute: clipboard is empty");
        return;
    };
    let Some(text) = clipboard.text() else {
        tracing::warn!("ImportCsvExecute: clipboard has no text content");
        return;
    };
    if text.trim().is_empty() {
        tracing::warn!("ImportCsvExecute: clipboard text is empty");
        return;
    }

    // Read table name from editor selection
    let table_name = workspace
        .active_item(cx)
        .and_then(|item| item.act_as::<Editor>(cx))
        .and_then(|editor| get_sql_from_editor(&editor, cx))
        .unwrap_or_else(|| "your_table".to_string());
    let table_name = table_name.trim().to_string();

    let mut lines = text.lines();
    let Some(header_line) = lines.next() else {
        return;
    };

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

    let mut all_sql = String::new();
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
        all_sql.push_str(&format!(
            "INSERT INTO {table_name} ({col_list}) VALUES ({});\n",
            value_list.join(", ")
        ));
        row_count += 1;
    }

    if row_count == 0 {
        return;
    }

    // Execute the inserts
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(session) = conn_panel.read(cx).session() else {
        tracing::warn!("ImportCsvExecute: no active database connection");
        return;
    };
    let runtime = conn_panel.read(cx).runtime();

    workspace.open_panel::<ResultPanel>(window, cx);
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        result_panel.update(cx, |panel, cx| {
            panel.execute_query(all_sql, session, runtime, cx);
        });
    }
}

/// Generate a SELECT query with JOINs based on foreign keys from the schema tree.
fn snippet_select_join_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(schema_tree) = conn_panel.read(cx).schema_tree() else {
        insert_into_editor(
            workspace,
            "-- Connect to a database first to generate JOINs",
            window,
            cx,
        );
        return;
    };

    // Find the first table with foreign keys and generate JOINs
    let mut sql =
        String::from("-- Auto-generated JOIN query from foreign keys\nSELECT\n    *\nFROM\n");
    let mut found = false;

    for schema in &schema_tree.schemas {
        for table in &schema.tables {
            if !table.foreign_keys.is_empty() {
                sql.push_str(&format!(
                    "    \"{}\".\"{}\" t1\n",
                    schema.info.name, table.info.name
                ));

                for fk in &table.foreign_keys {
                    let col = fk.columns.first().map(|s| s.as_str()).unwrap_or("?");
                    let ref_col = fk
                        .referenced_columns
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("?");
                    sql.push_str(&format!(
                        "    LEFT JOIN \"{}\" ON t1.\"{}\" = \"{}\".\"{}\"  -- {}\n",
                        fk.referenced_table, col, fk.referenced_table, ref_col, fk.name
                    ));
                }
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }

    if !found {
        sql.push_str("    your_table t1\n    -- No foreign keys found in schema\n");
    }

    sql.push_str("LIMIT 100;");
    insert_into_editor(workspace, &sql, window, cx);
}

/// Generate a GROUP BY summary template for the selected table name.
fn snippet_group_summary_action(
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
    let Some(table_name) = get_sql_from_editor(&editor, cx) else {
        return;
    };
    let table = table_name.trim();

    let sql = format!(
        "-- Summary statistics for {table}\nSELECT\n    \
        COUNT(*) AS total_rows,\n    \
        COUNT(DISTINCT column_name) AS distinct_values,\n    \
        MIN(column_name) AS min_value,\n    \
        MAX(column_name) AS max_value,\n    \
        AVG(numeric_column::numeric) AS avg_value\n\
        FROM {table}\n-- GROUP BY grouping_column\n-- HAVING COUNT(*) > 1\n\
        ORDER BY total_rows DESC;"
    );
    insert_into_editor(workspace, &sql, window, cx);
}

/// Run a comprehensive database health check and display results.
fn health_check_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "WITH db_size AS ( \
        SELECT pg_size_pretty(pg_database_size(current_database())) AS database_size \
    ), \
    connections AS ( \
        SELECT count(*) AS total_connections, \
            count(*) FILTER (WHERE state = 'active') AS active, \
            count(*) FILTER (WHERE state = 'idle') AS idle, \
            count(*) FILTER (WHERE state = 'idle in transaction') AS idle_in_tx \
        FROM pg_stat_activity WHERE datname = current_database() \
    ), \
    cache AS ( \
        SELECT CASE WHEN sum(blks_hit) + sum(blks_read) > 0 \
            THEN round(100.0 * sum(blks_hit) / (sum(blks_hit) + sum(blks_read)), 2) \
            ELSE 0 END AS cache_hit_pct \
        FROM pg_stat_database WHERE datname = current_database() \
    ), \
    tables AS ( \
        SELECT count(*) AS table_count, \
            sum(n_dead_tup) AS total_dead_tuples, \
            count(*) FILTER (WHERE n_dead_tup > n_live_tup * 0.2) AS tables_need_vacuum \
        FROM pg_stat_user_tables \
    ), \
    indexes AS ( \
        SELECT count(*) AS total_indexes, \
            count(*) FILTER (WHERE idx_scan = 0 AND indexrelname NOT LIKE '%_pkey') AS unused_indexes \
        FROM pg_stat_user_indexes \
    ) \
    SELECT \
        'Database Size' AS metric, db_size.database_size AS value FROM db_size \
    UNION ALL SELECT 'Total Connections', connections.total_connections::text FROM connections \
    UNION ALL SELECT 'Active Queries', connections.active::text FROM connections \
    UNION ALL SELECT 'Idle Connections', connections.idle::text FROM connections \
    UNION ALL SELECT 'Idle in Transaction', connections.idle_in_tx::text FROM connections \
    UNION ALL SELECT 'Cache Hit Rate %', cache.cache_hit_pct::text FROM cache \
    UNION ALL SELECT 'User Tables', tables.table_count::text FROM tables \
    UNION ALL SELECT 'Dead Tuples', tables.total_dead_tuples::text FROM tables \
    UNION ALL SELECT 'Tables Need VACUUM', tables.tables_need_vacuum::text FROM tables \
    UNION ALL SELECT 'Total Indexes', indexes.total_indexes::text FROM indexes \
    UNION ALL SELECT 'Unused Indexes', indexes.unused_indexes::text FROM indexes";

    execute_system_query(workspace, sql, window, cx);
}

/// Copy the selected text as a properly quoted SQL identifier to the clipboard.
fn copy_qualified_name_action(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(selected) = get_sql_from_editor(&editor, cx) else {
        return;
    };
    let trimmed = selected.trim();
    if trimmed.is_empty() {
        return;
    }

    // Quote it as a SQL identifier
    let qualified = if trimmed.contains('.') {
        let parts: Vec<&str> = trimmed.split('.').collect();
        parts
            .iter()
            .map(|p| format!("\"{}\"", p.trim_matches('"')))
            .collect::<Vec<_>>()
            .join(".")
    } else {
        format!("\"{}\"", trimmed.trim_matches('"'))
    };

    cx.write_to_clipboard(ClipboardItem::new_string(qualified));
}

/// Generate GRANT/REVOKE statements for the selected table name.
fn generate_grant_action(
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
    let table = selected.trim();

    let sql = format!(
        "-- Grant permissions on {table}\n\
         GRANT SELECT ON {table} TO role_name;\n\
         GRANT INSERT, UPDATE, DELETE ON {table} TO role_name;\n\
         GRANT ALL PRIVILEGES ON {table} TO role_name;\n\n\
         -- Revoke permissions\n\
         REVOKE ALL PRIVILEGES ON {table} FROM role_name;\n\n\
         -- Grant with grant option\n\
         GRANT SELECT ON {table} TO role_name WITH GRANT OPTION;"
    );
    insert_into_editor(workspace, &sql, window, cx);
}

/// Insert common PostgreSQL admin commands into the active editor.
fn admin_commands_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let sql = "-- Common PostgreSQL Admin Commands\n\n\
        -- Terminate all connections to a database\n\
        SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'database_name' AND pid <> pg_backend_pid();\n\n\
        -- Reload PostgreSQL configuration\n\
        SELECT pg_reload_conf();\n\n\
        -- Check if PostgreSQL is in recovery\n\
        SELECT pg_is_in_recovery();\n\n\
        -- Current WAL position\n\
        SELECT pg_current_wal_lsn();\n\n\
        -- Cancel a running query (graceful)\n\
        SELECT pg_cancel_backend(pid);\n\n\
        -- Terminate a backend (forceful)\n\
        SELECT pg_terminate_backend(pid);\n\n\
        -- List all databases with sizes\n\
        SELECT datname, pg_size_pretty(pg_database_size(datname)) FROM pg_database ORDER BY pg_database_size(datname) DESC;\n\n\
        -- Show current connections per database\n\
        SELECT datname, count(*) FROM pg_stat_activity GROUP BY datname ORDER BY count(*) DESC;\n\n\
        -- Reset statistics\n\
        SELECT pg_stat_reset();";
    insert_into_editor(workspace, sql, window, cx);
}

// ---------------------------------------------------------------------------
// Feature 1: .env File Integration
// ---------------------------------------------------------------------------

struct ParsedDatabaseUrl {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
    sslmode: bool,
}

/// Parse a postgres:// or postgresql:// connection URL into its components.
fn parse_database_url(url: &str) -> Option<ParsedDatabaseUrl> {
    let url = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))?;

    let (auth, rest) = url.split_once('@')?;
    let (username, password) = auth.split_once(':').unwrap_or((auth, ""));

    let (host_port, db_params) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port_str) = host_port.split_once(':').unwrap_or((host_port, "5432"));
    let port: u16 = port_str.parse().unwrap_or(5432);

    let (database, params) = db_params.split_once('?').unwrap_or((db_params, ""));
    let sslmode = params.contains("sslmode=require");

    Some(ParsedDatabaseUrl {
        host: host.to_string(),
        port,
        database: database.to_string(),
        username: username.to_string(),
        password: password.to_string(),
        sslmode,
    })
}

/// Detect DATABASE_URL from .env files in the current working directory and auto-connect.
fn detect_env_database_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
                workspace.open_panel::<ResultPanel>(window, cx);
                result_panel.update(cx, |panel, cx| {
                    panel.show_ddl(
                        "Could not determine current working directory.".to_string(),
                        cx,
                    );
                });
            }
            return;
        }
    };

    let env_files = [".env", ".env.local", ".env.development", ".env.production"];
    let db_keys = [
        "DATABASE_URL",
        "DB_URL",
        "POSTGRES_URL",
        "PG_URL",
        "POSTGRESQL_URL",
    ];

    let mut database_urls: Vec<(String, String, String)> = Vec::new();

    for env_file in &env_files {
        let env_path = cwd.join(env_file);
        if let Ok(content) = std::fs::read_to_string(&env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }

                for key in &db_keys {
                    if let Some(value) = line.strip_prefix(&format!("{key}=")) {
                        let url = value.trim().trim_matches('"').trim_matches('\'');
                        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                            database_urls.push((
                                env_file.to_string(),
                                key.to_string(),
                                url.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    if database_urls.is_empty() {
        if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
            workspace.open_panel::<ResultPanel>(window, cx);
            result_panel.update(cx, |panel, cx| {
                panel.show_ddl(
                    "No DATABASE_URL found in .env files.\n\n\
                     Checked: .env, .env.local, .env.development, .env.production\n\
                     Looked for: DATABASE_URL, DB_URL, POSTGRES_URL, PG_URL, POSTGRESQL_URL"
                        .to_string(),
                    cx,
                );
            });
        }
        return;
    }

    let (file, key, url) = &database_urls[0];

    if let Some(parsed) = parse_database_url(url) {
        let info = format!(
            "Found {} database URL(s) in project:\n\n{}\n\n\
             Parsed from {} in {}:\n  Host: {}\n  Port: {}\n  Database: {}\n  Username: {}",
            database_urls.len(),
            database_urls
                .iter()
                .map(|(f, k, _)| format!("  {f}: {k}"))
                .collect::<Vec<_>>()
                .join("\n"),
            key,
            file,
            parsed.host,
            parsed.port,
            parsed.database,
            parsed.username
        );

        // Auto-connect via the ConnectionPanel
        if let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) {
            let profile = ConnectionProfile {
                id: ConnectionId::new(),
                name: format!("{}@{} (from .env)", parsed.database, parsed.host),
                host: parsed.host.clone(),
                port: parsed.port,
                database: parsed.database.clone(),
                username: parsed.username.clone(),
                environment: if file.contains("production") {
                    Environment::Production
                } else {
                    Environment::Development
                },
                ssl_mode: if parsed.sslmode {
                    SslMode::Require
                } else {
                    SslMode::Disable
                },
                read_only_default: file.contains("production"),
                ssh: SshConfig::default(),
            };

            conn_panel.update(cx, |panel, cx| {
                panel.connect(profile, parsed.password.clone(), cx);
            });
        }

        // Show info in result panel
        if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
            workspace.open_panel::<ResultPanel>(window, cx);
            result_panel.update(cx, |panel, cx| {
                panel.show_ddl(info, cx);
            });
        }
    } else if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        workspace.open_panel::<ResultPanel>(window, cx);
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(
                format!("Found {key} in {file} but could not parse the URL."),
                cx,
            );
        });
    }
}

// ---------------------------------------------------------------------------
// Feature 3: Query Timeout Configuration
// ---------------------------------------------------------------------------

/// Set statement_timeout for the current session. Reads timeout value from
/// the active editor selection (in seconds or PostgreSQL interval notation).
fn set_timeout_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(selected) = get_sql_from_editor(&editor, cx) else {
        // Default to 30 seconds if nothing is selected
        let sql = "SET statement_timeout = '30s'";
        execute_system_query(workspace, sql, window, cx);
        return;
    };
    let timeout = selected.trim();

    let sql = if timeout.parse::<u32>().is_ok() {
        format!("SET statement_timeout = '{}s'", timeout)
    } else {
        format!("SET statement_timeout = '{}'", timeout)
    };

    execute_system_query(workspace, &sql, window, cx);
}

// ---------------------------------------------------------------------------
// Feature: SQL Linting (Common Anti-Pattern Detection)
// ---------------------------------------------------------------------------

/// Analyze the current SQL for common anti-patterns and display warnings.
/// This is a local analysis that does not require a database connection.
fn lint_sql_action(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let Some(sql) = get_sql_from_editor(&editor, cx) else {
        return;
    };

    let mut warnings: Vec<(String, String, String)> = Vec::new();
    let upper = sql.to_uppercase();

    // SELECT *
    if upper.contains("SELECT *") && !upper.contains("COUNT(*)") && !upper.contains("EXISTS") {
        warnings.push((
            "SELECT *".to_string(),
            "Specify columns explicitly. SELECT * fetches unnecessary data and breaks if schema changes.".to_string(),
            "MEDIUM".to_string(),
        ));
    }

    // DELETE/UPDATE without WHERE
    if (upper.contains("DELETE FROM") || upper.starts_with("UPDATE")) && !upper.contains("WHERE") {
        warnings.push((
            "Missing WHERE clause".to_string(),
            "DELETE/UPDATE without WHERE affects ALL rows. This is usually a mistake.".to_string(),
            "CRITICAL".to_string(),
        ));
    }

    // LIKE with leading wildcard
    if upper.contains("LIKE '%") || upper.contains("ILIKE '%") {
        warnings.push((
            "Leading wildcard in LIKE".to_string(),
            "LIKE '%value' cannot use indexes. Consider full-text search (tsvector) or trigram indexes (pg_trgm).".to_string(),
            "HIGH".to_string(),
        ));
    }

    // Implicit type casting in WHERE
    if upper.contains("::TEXT") && upper.contains("WHERE") {
        warnings.push((
            "Type casting in WHERE".to_string(),
            "Casting columns in WHERE clauses prevents index usage. Cast the parameter instead."
                .to_string(),
            "MEDIUM".to_string(),
        ));
    }

    // NOT IN with subquery
    if upper.contains("NOT IN (SELECT") {
        warnings.push((
            "NOT IN with subquery".to_string(),
            "NOT IN returns no rows if any value is NULL. Use NOT EXISTS instead.".to_string(),
            "HIGH".to_string(),
        ));
    }

    // ORDER BY without LIMIT
    if upper.contains("ORDER BY") && !upper.contains("LIMIT") && !upper.contains("FETCH") {
        warnings.push((
            "ORDER BY without LIMIT".to_string(),
            "Sorting entire result set is expensive. Add LIMIT for pagination.".to_string(),
            "LOW".to_string(),
        ));
    }

    // Cartesian join (FROM a, b without WHERE)
    if upper.contains("FROM") && !upper.contains("JOIN") && !upper.contains("WHERE") {
        if let Some(from_idx) = upper.find("FROM") {
            let after_from = &upper[from_idx..];
            if after_from.contains(',') {
                warnings.push((
                    "Possible Cartesian join".to_string(),
                    "Multiple tables in FROM without WHERE produces a cross product. Use explicit JOIN.".to_string(),
                    "HIGH".to_string(),
                ));
            }
        }
    }

    // DISTINCT with JOIN
    if upper.contains("SELECT DISTINCT") && upper.contains("JOIN") {
        warnings.push((
            "DISTINCT with JOIN".to_string(),
            "DISTINCT after JOIN often indicates a join producing duplicates. Check join conditions.".to_string(),
            "MEDIUM".to_string(),
        ));
    }

    // Deep subquery nesting
    let subquery_count = upper.matches("(SELECT").count();
    if subquery_count > 2 {
        let subquery_msg = format!(
            "{subquery_count} nested subqueries detected. Consider using CTEs (WITH clause) for readability."
        );
        warnings.push((
            "Deep subquery nesting".to_string(),
            subquery_msg,
            "MEDIUM".to_string(),
        ));
    }

    // COUNT(*) without WHERE on entire table
    if upper.contains("COUNT(*)") && !upper.contains("WHERE") && !upper.contains("GROUP BY") {
        warnings.push((
            "Unfiltered COUNT(*)".to_string(),
            "COUNT(*) without WHERE scans the entire table. This is slow on large tables."
                .to_string(),
            "LOW".to_string(),
        ));
    }

    // Build output
    let mut output = format!(
        "=== SQL Lint Report ===\n{} issue(s) found\n\n",
        warnings.len()
    );

    if warnings.is_empty() {
        output.push_str("No issues detected. Query looks good!\n");
    } else {
        for (i, (title, desc, severity)) in warnings.iter().enumerate() {
            output.push_str(&format!(
                "{}. [{}] {}\n   {}\n\n",
                i + 1,
                severity,
                title,
                desc
            ));
        }

        output.push_str("---\nSeverity levels:\n");
        output.push_str("  CRITICAL: Will cause data loss or incorrect results\n");
        output.push_str("  HIGH: Performance or correctness issue\n");
        output.push_str("  MEDIUM: Could be improved\n");
        output.push_str("  LOW: Suggestion\n");
    }

    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        workspace.open_panel::<ResultPanel>(window, cx);
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(output, cx);
        });
    }
}

// ---------------------------------------------------------------------------
// Feature: Query Plan Comparison
// ---------------------------------------------------------------------------

/// Run EXPLAIN ANALYZE on two queries separated by `---` and show both plans
/// side by side for comparison.
fn compare_explain_action(
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
    let sql = editor.read(cx).text(cx);

    // Split by "---" separator
    let parts: Vec<&str> = sql.split("---").collect();
    if parts.len() < 2 {
        if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
            workspace.open_panel::<ResultPanel>(window, cx);
            result_panel.update(cx, |panel, cx| {
                panel.show_ddl(
                    "To compare query plans, write two queries separated by ---\n\n\
                     Example:\n\
                     SELECT * FROM users WHERE email = 'test@test.com'\n\
                     ---\n\
                     SELECT * FROM users WHERE id = 1"
                        .to_string(),
                    cx,
                );
            });
        }
        return;
    }

    let query1 = parts[0].trim().to_string();
    let query2 = parts[1].trim().to_string();

    if query1.is_empty() || query2.is_empty() {
        return;
    }

    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(session) = conn_panel.read(cx).session() else {
        tracing::warn!("CompareExplain: no active database connection");
        return;
    };
    let runtime = conn_panel.read(cx).runtime();

    let explain1 = format!(
        "EXPLAIN (ANALYZE, COSTS, BUFFERS, FORMAT JSON) {}",
        query1.trim_end_matches(';')
    );
    let explain2 = format!(
        "EXPLAIN (ANALYZE, COSTS, BUFFERS, FORMAT JSON) {}",
        query2.trim_end_matches(';')
    );

    workspace.open_panel::<ResultPanel>(window, cx);
    let Some(result_panel) = workspace.panel::<ResultPanel>(cx) else {
        return;
    };

    result_panel.update(cx, |panel, cx| {
        // Show loading
        panel.show_ddl("Comparing query plans...".to_string(), cx);

        let tab_idx = panel.tabs.len() - 1;

        let session1: Arc<dyn DatabaseSession> = session.clone();
        let session2: Arc<dyn DatabaseSession> = session;
        let runtime1: Arc<Runtime> = runtime.clone();
        let runtime2: Arc<Runtime> = runtime;
        let q1 = query1.clone();
        let q2 = query2.clone();

        cx.spawn(async move |this, cx| {
            let result1 = runtime1
                .spawn({
                    let session = session1;
                    let sql = explain1;
                    async move { session.execute(&sql).await }
                })
                .await;

            let result2 = runtime2
                .spawn({
                    let session = session2;
                    let sql = explain2;
                    async move { session.execute(&sql).await }
                })
                .await;

            let plan1 = match result1 {
                Ok(Ok(rs)) => rs
                    .rows
                    .first()
                    .and_then(|r| r.first())
                    .map(|c| c.display())
                    .unwrap_or_else(|| "Plan 1 failed".to_string()),
                Ok(Err(e)) => format!("Plan 1 error: {e}"),
                Err(e) => format!("Plan 1 execution failed: {e}"),
            };

            let plan2 = match result2 {
                Ok(Ok(rs)) => rs
                    .rows
                    .first()
                    .and_then(|r| r.first())
                    .map(|c| c.display())
                    .unwrap_or_else(|| "Plan 2 failed".to_string()),
                Ok(Err(e)) => format!("Plan 2 error: {e}"),
                Err(e) => format!("Plan 2 execution failed: {e}"),
            };

            let formatted1 = result_panel::format_explain_plan(&plan1);
            let formatted2 = result_panel::format_explain_plan(&plan2);

            let comparison = format!(
                "=== Query Plan Comparison ===\n\n\
                 --- Query 1 ---\n{}\n\n\
                 --- Plan 1 ---\n{}\n\n\
                 ===============\n\n\
                 --- Query 2 ---\n{}\n\n\
                 --- Plan 2 ---\n{}\n",
                q1, formatted1, q2, formatted2
            );

            this.update(cx, |panel, cx| {
                if let Some(tab) = panel.tabs.get_mut(tab_idx) {
                    tab.state = result_panel::ResultState::Ddl(comparison);
                    tab.label = "Plan Compare".to_string();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    });
}

// ---------------------------------------------------------------------------
// Feature: Explain This Query (AI Context Preparation)
// ---------------------------------------------------------------------------

/// Prepare AI context for query analysis and copy to clipboard.
/// Includes the SQL query and available schema information so the user
/// can paste it into Zed's AI assistant for analysis.
fn explain_query_ai_action(
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
    let Some(sql) = get_sql_from_editor(&editor, cx) else {
        return;
    };

    // Build context with schema information
    let mut context =
        format!("Please explain and optimize this PostgreSQL query:\n\n```sql\n{sql}\n```\n\n");

    // Add schema context if available
    if let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) {
        if let Some(schema_tree) = conn_panel.read(cx).schema_tree() {
            context.push_str("## Available Schema:\n\n");
            for schema in &schema_tree.schemas {
                for table in &schema.tables {
                    context.push_str(&format!("### {}.{}\n", schema.info.name, table.info.name));
                    for col in &table.columns {
                        let pk = if col.is_primary_key { " (PK)" } else { "" };
                        context.push_str(&format!("- {} {}{}\n", col.name, col.data_type, pk));
                    }
                    if !table.indexes.is_empty() {
                        context.push_str("Indexes:\n");
                        for idx in &table.indexes {
                            let unique = if idx.is_unique { "UNIQUE " } else { "" };
                            context.push_str(&format!(
                                "- {}{} on ({})\n",
                                unique,
                                idx.name,
                                idx.columns.join(", ")
                            ));
                        }
                    }
                    context.push('\n');
                }
            }
        }
    }

    context.push_str(
        "\nPlease:\n\
         1. Explain what this query does\n\
         2. Identify any performance issues\n\
         3. Suggest optimizations\n\
         4. Recommend missing indexes if applicable\n",
    );

    // Copy to clipboard so user can paste into Zed's AI assistant
    cx.write_to_clipboard(ClipboardItem::new_string(context.clone()));

    // Show in result panel
    if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
        workspace.open_panel::<ResultPanel>(window, cx);
        result_panel.update(cx, |panel, cx| {
            panel.show_ddl(
                format!(
                    "AI context copied to clipboard!\n\n\
                     Open Zed's AI assistant (Cmd+;) and paste to get an analysis.\n\n\
                     ---\n\n{context}"
                ),
                cx,
            );
        });
    }
}
