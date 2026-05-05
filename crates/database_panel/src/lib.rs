mod connection_panel;
mod result_panel;

use editor::Editor;
use editor::actions::SelectAll;
use gpui::{App, AppContext as _, Context, Window, actions};
use workspace::Workspace;

pub use connection_panel::ConnectionPanel;
pub use result_panel::ResultPanel;

actions!(
    database_panel,
    [ExecuteQuery, ViewSessions, ExplainAnalyze, FormatSql]
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

fn execute_query_action(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    // Get SQL from the active editor
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let sql = editor.read(cx).text(cx);
    if sql.trim().is_empty() {
        return;
    }

    // Get session from ConnectionPanel
    let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
        return;
    };
    let Some(session) = conn_panel.read(cx).session() else {
        tracing::warn!("ExecuteQuery: no active database connection");
        return;
    };
    let runtime = conn_panel.read(cx).runtime();

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
    // Get SQL from the active editor
    let Some(active_item) = workspace.active_item(cx) else {
        return;
    };
    let Some(editor) = active_item.act_as::<Editor>(cx) else {
        return;
    };
    let sql = editor.read(cx).text(cx);
    if sql.trim().is_empty() {
        return;
    }

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
