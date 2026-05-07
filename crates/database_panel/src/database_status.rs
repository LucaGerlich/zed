use gpui::{Context, Empty, IntoElement, ParentElement, Render, Subscription, WeakEntity, Window};
use ui::{ButtonLike, Color, Icon, IconName, IconSize, Label, LabelSize, Tooltip, prelude::*};
use workspace::{StatusItemView, Workspace, item::ItemHandle};

use pgblade_core::connection::Environment;

use crate::ConnectionPanel;

/// A status bar item that displays the currently connected database.
///
/// Shows a small database icon with the `database@host` label when
/// connected, and a muted "No DB" label when disconnected.
pub struct DatabaseStatusItem {
    workspace: WeakEntity<Workspace>,
    _observe_connection_panel: Option<Subscription>,
}

impl DatabaseStatusItem {
    pub fn new(workspace: &Workspace, cx: &mut Context<Self>) -> Self {
        let weak = workspace.weak_handle();
        let subscription = workspace
            .panel::<ConnectionPanel>(cx)
            .map(|panel| cx.observe(&panel, |_, _, cx| cx.notify()));

        Self {
            workspace: weak,
            _observe_connection_panel: subscription,
        }
    }
}

impl Render for DatabaseStatusItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = self.workspace.upgrade() else {
            return Empty.into_any_element();
        };

        let workspace = workspace.read(cx);
        let Some(conn_panel) = workspace.panel::<ConnectionPanel>(cx) else {
            return Empty.into_any_element();
        };

        let panel = conn_panel.read(cx);
        let Some(status_text) = panel.status_bar_text() else {
            // Not connected -- show a subtle disconnected indicator
            return h_flex()
                .gap_1()
                .child(
                    Icon::new(IconName::DatabaseZap)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    Label::new("No DB")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element();
        };

        // Determine icon color based on connection environment
        let env = panel.connection_environment();
        let icon_color = match env {
            Some(Environment::Production) => Color::Error,
            Some(Environment::Staging) => Color::Warning,
            _ => Color::Success,
        };
        let env_label = env.map(|e| e.label()).unwrap_or("Unknown");

        // Connected: show database@host with an environment-colored icon and a tooltip
        let tooltip_text: SharedString = format!("Connected: {status_text} ({env_label})").into();
        ButtonLike::new("database-status")
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::DatabaseZap)
                            .size(IconSize::Small)
                            .color(icon_color),
                    )
                    .child(
                        Label::new(status_text)
                            .size(LabelSize::Small)
                            .color(Color::Default),
                    ),
            )
            .tooltip(Tooltip::text(tooltip_text))
            .into_any_element()
    }
}

impl StatusItemView for DatabaseStatusItem {
    fn set_active_pane_item(
        &mut self,
        _active_pane_item: Option<&dyn ItemHandle>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Re-subscribe to the connection panel if it was not yet available
        // when this status item was first created (panels may initialize late).
        if self._observe_connection_panel.is_none() {
            if let Some(workspace) = self.workspace.upgrade() {
                let workspace = workspace.read(cx);
                if let Some(panel) = workspace.panel::<ConnectionPanel>(cx) {
                    self._observe_connection_panel =
                        Some(cx.observe(&panel, |_, _, cx| cx.notify()));
                }
            }
        }
        cx.notify();
    }
}
