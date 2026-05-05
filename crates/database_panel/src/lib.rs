mod connection_panel;
mod result_panel;

use gpui::{App, AppContext, Context, Window};
use workspace::Workspace;

pub use connection_panel::ConnectionPanel;
pub use result_panel::ResultPanel;

pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, window: Option<&mut Window>, cx: &mut Context<Workspace>| {
            connection_panel::register(workspace);
            result_panel::register(workspace);

            if let Some(window) = window {
                let connection = cx.new(|cx| ConnectionPanel::new(window, cx));
                workspace.add_panel(connection, window, cx);

                let results = cx.new(|cx| ResultPanel::new(cx));
                workspace.add_panel(results, window, cx);
            }
        },
    )
    .detach();
}
