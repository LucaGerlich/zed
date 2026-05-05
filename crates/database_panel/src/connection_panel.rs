use editor::Editor;
use gpui::*;
use ui::prelude::*;
use ui::{Button, ButtonStyle, IconName};
use workspace::Workspace;
use workspace::dock::{DockPosition, Panel, PanelEvent};

use pgblade_core::connection::{ConnectionProfile, Environment};
use pgblade_core::security::CredentialStore;
use pgblade_core::storage::StorageManager;
use pgblade_security::KeychainStore;

actions!(database_panel, [ToggleFocus, AddConnection]);

pub fn register(workspace: &mut Workspace) {
    workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
        workspace.toggle_panel_focus::<ConnectionPanel>(window, cx);
    });
}

pub struct ConnectionPanel {
    focus_handle: FocusHandle,
    active: bool,
    saved_connections: Vec<ConnectionProfile>,
    show_form: bool,
    // Form editors
    host_editor: Entity<Editor>,
    port_editor: Entity<Editor>,
    database_editor: Entity<Editor>,
    username_editor: Entity<Editor>,
    password_editor: Entity<Editor>,
    form_environment: Environment,
    // Storage
    storage: StorageManager,
    credential_store: KeychainStore,
}

impl ConnectionPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let storage = StorageManager::init().unwrap_or_else(|e| {
            tracing::error!("failed to init storage: {e}");
            StorageManager::in_memory().unwrap()
        });

        let saved = storage.load_connections().unwrap_or_default();

        let host_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("localhost", window, cx);
            editor.set_text("localhost", window, cx);
            editor
        });

        let port_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("5432", window, cx);
            editor.set_text("5432", window, cx);
            editor
        });

        let database_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("database name", window, cx);
            editor
        });

        let username_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("postgres", window, cx);
            editor.set_text("postgres", window, cx);
            editor
        });

        let password_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("password", window, cx);
            editor.set_masked(true, cx);
            editor
        });

        Self {
            focus_handle: cx.focus_handle(),
            active: false,
            saved_connections: saved,
            show_form: false,
            host_editor,
            port_editor,
            database_editor,
            username_editor,
            password_editor,
            form_environment: Environment::Local,
            storage,
            credential_store: KeychainStore::new(),
        }
    }

    fn toggle_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_form = !self.show_form;
        if self.show_form {
            // Reset editors to defaults
            self.host_editor.update(cx, |editor, cx| {
                editor.set_text("localhost", window, cx);
            });
            self.port_editor.update(cx, |editor, cx| {
                editor.set_text("5432", window, cx);
            });
            self.database_editor.update(cx, |editor, cx| {
                editor.set_text("", window, cx);
            });
            self.username_editor.update(cx, |editor, cx| {
                editor.set_text("postgres", window, cx);
            });
            self.password_editor.update(cx, |editor, cx| {
                editor.set_text("", window, cx);
            });
            self.form_environment = Environment::Local;
        }
        cx.notify();
    }

    fn save_connection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host_editor.read(cx).text(cx);
        let port_str = self.port_editor.read(cx).text(cx);
        let database = self.database_editor.read(cx).text(cx);
        let username = self.username_editor.read(cx).text(cx);
        let password = self.password_editor.read(cx).text(cx);

        let profile = ConnectionProfile {
            id: pgblade_core::connection::ConnectionId::new(),
            name: format!("{}@{}", database, host),
            host,
            port: port_str.parse().unwrap_or(5432),
            database,
            username,
            environment: self.form_environment,
            ssl_mode: pgblade_core::connection::SslMode::Disable,
            read_only_default: self.form_environment.is_production(),
        };

        // Check for duplicates
        if let Ok(Some(existing_id)) = self.storage.find_duplicate(&profile) {
            let mut updated = profile.clone();
            updated.id = existing_id;
            let _ = self.storage.save_connection(&updated);
            let _ = self
                .credential_store
                .store(&updated.keychain_service_key(), &password);
        } else {
            let _ = self.storage.save_connection(&profile);
            let _ = self
                .credential_store
                .store(&profile.keychain_service_key(), &password);
        }

        self.saved_connections = self.storage.load_connections().unwrap_or_default();
        self.show_form = false;
        cx.notify();
    }

    fn delete_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(conn) = self.saved_connections.get(index) {
            let id = conn.id;
            let key = conn.keychain_service_key();
            let _ = self.storage.delete_connection(&id);
            let _ = self.credential_store.delete(&key);
            self.saved_connections = self.storage.load_connections().unwrap_or_default();
            cx.notify();
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_2()
            .py_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().colors().text_muted)
                    .child("DATABASE NAVIGATOR"),
            )
            .child(
                div()
                    .id("add-connection-btn")
                    .cursor_pointer()
                    .text_sm()
                    .text_color(cx.theme().colors().text_muted)
                    .hover(|s| s.text_color(cx.theme().colors().text))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_form(window, cx);
                    }))
                    .child("+"),
            )
    }

    fn render_connection_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col();

        if self.saved_connections.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child("No connections yet"),
            );
        }

        for (i, conn) in self.saved_connections.iter().enumerate() {
            let env_color = match conn.environment {
                Environment::Production => rgb(0xf14c4c),
                Environment::Staging => rgb(0xccaa00),
                _ => rgb(0x4ec94e),
            };
            let name = conn.name.clone();
            let idx = i;

            list = list.child(
                div()
                    .id(SharedString::from(format!("conn-{i}")))
                    .h(px(26.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .rounded_sm()
                    .hover(|s| s.bg(cx.theme().colors().element_active))
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(env_color)
                            .mr_2()
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().colors().text)
                            .child(name),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("del-conn-{i}")))
                            .text_xs()
                            .text_color(cx.theme().colors().text_muted)
                            .hover(|s| s.text_color(gpui::red()))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.delete_connection(idx, cx);
                            }))
                            .child("x"),
                    ),
            );
        }

        list
    }

    fn render_editor_field(
        &self,
        label: &str,
        editor: &Entity<Editor>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_1()
                    .bg(cx.theme().colors().editor_background)
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .rounded_sm()
                    .child(editor.clone()),
            )
    }

    fn render_environment_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let environments = [
            (Environment::Local, "Local"),
            (Environment::Development, "Dev"),
            (Environment::Staging, "Staging"),
            (Environment::Production, "Prod"),
        ];

        let mut row = div().flex().flex_row().gap_1();

        for (env, label) in environments {
            let is_selected = self.form_environment == env;
            row = row.child(
                div()
                    .id(SharedString::from(format!("env-{label}")))
                    .px_2()
                    .py(px(2.))
                    .text_xs()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(is_selected, |s| {
                        s.bg(cx.theme().colors().element_active)
                            .text_color(cx.theme().colors().text)
                    })
                    .when(!is_selected, |s| {
                        s.text_color(cx.theme().colors().text_muted)
                            .hover(|s| s.bg(cx.theme().colors().element_active))
                    })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.form_environment = env;
                        cx.notify();
                    }))
                    .child(label),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child("Environment"),
            )
            .child(row)
    }

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().surface_background)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().colors().text)
                    .child("New Connection"),
            )
            .child(self.render_editor_field("Host", &self.host_editor, cx))
            .child(self.render_editor_field("Port", &self.port_editor, cx))
            .child(self.render_editor_field("Database", &self.database_editor, cx))
            .child(self.render_editor_field("Username", &self.username_editor, cx))
            .child(self.render_editor_field("Password", &self.password_editor, cx))
            .child(self.render_environment_selector(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.show_form = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("save-connect", "Save & Connect")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_connection(window, cx);
                            })),
                    ),
            )
    }
}

impl Focusable for ConnectionPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PanelEvent> for ConnectionPanel {}

impl Render for ConnectionPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .id("connection-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_header(cx))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .py_1()
                    .child(self.render_connection_list(cx)),
            );

        if self.show_form {
            panel = panel.child(self.render_form(cx));
        }

        panel
    }
}

impl Panel for ConnectionPanel {
    fn persistent_name() -> &'static str {
        "DatabasePanel"
    }

    fn panel_key() -> &'static str {
        "DatabasePanel"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Left
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn default_size(&self, _window: &Window, _cx: &App) -> Pixels {
        px(280.)
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<ui::IconName> {
        Some(IconName::DatabaseZap)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Database Panel")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleFocus)
    }

    fn activation_priority(&self) -> u32 {
        3
    }

    fn starts_open(&self, _window: &Window, _cx: &App) -> bool {
        false
    }

    fn set_active(&mut self, active: bool, _window: &mut Window, _cx: &mut Context<Self>) {
        self.active = active;
    }
}
