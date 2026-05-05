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
        Disconnect
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
