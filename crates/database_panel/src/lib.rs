mod connection_panel;
mod result_panel;

use editor::Editor;
use gpui::{App, AppContext as _, Context, Window, actions};
use workspace::Workspace;

pub use connection_panel::ConnectionPanel;
pub use result_panel::ResultPanel;

actions!(database_panel, [ExecuteQuery]);

pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, window: Option<&mut Window>, cx: &mut Context<Workspace>| {
            connection_panel::register(workspace);
            result_panel::register(workspace);

            // Register the ExecuteQuery action on the workspace
            workspace.register_action(|workspace, _: &ExecuteQuery, window, cx| {
                execute_query_action(workspace, window, cx);
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
