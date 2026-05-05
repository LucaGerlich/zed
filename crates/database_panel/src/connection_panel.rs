use gpui::*;
use ui::IconName;
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

/// State for the connection form shown inline in the panel.
struct ConnectionForm {
    host: String,
    port: String,
    database: String,
    username: String,
    password: String,
    environment: Environment,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: "5432".to_string(),
            database: String::new(),
            username: "postgres".to_string(),
            password: String::new(),
            environment: Environment::Local,
        }
    }
}

pub struct ConnectionPanel {
    focus_handle: FocusHandle,
    active: bool,
    _width: Option<Pixels>,
    saved_connections: Vec<ConnectionProfile>,
    show_form: bool,
    form: ConnectionForm,
    storage: StorageManager,
    credential_store: KeychainStore,
}

impl ConnectionPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let storage = StorageManager::init().unwrap_or_else(|e| {
            tracing::error!("failed to init storage: {e}");
            StorageManager::in_memory().unwrap()
        });

        let saved = storage.load_connections().unwrap_or_default();

        Self {
            focus_handle: cx.focus_handle(),
            active: false,
            _width: None,
            saved_connections: saved,
            show_form: false,
            form: ConnectionForm::default(),
            storage,
            credential_store: KeychainStore::new(),
        }
    }

    fn toggle_form(&mut self, cx: &mut Context<Self>) {
        self.show_form = !self.show_form;
        if self.show_form {
            self.form = ConnectionForm::default();
        }
        cx.notify();
    }

    fn save_connection(&mut self, cx: &mut Context<Self>) {
        let profile = ConnectionProfile {
            id: pgblade_core::connection::ConnectionId::new(),
            name: format!("{}@{}", self.form.database, self.form.host),
            host: self.form.host.clone(),
            port: self.form.port.parse().unwrap_or(5432),
            database: self.form.database.clone(),
            username: self.form.username.clone(),
            environment: self.form.environment,
            ssl_mode: pgblade_core::connection::SslMode::Disable,
            read_only_default: self.form.environment.is_production(),
        };

        // Check for duplicates
        if let Ok(Some(existing_id)) = self.storage.find_duplicate(&profile) {
            let mut updated = profile.clone();
            updated.id = existing_id;
            let _ = self.storage.save_connection(&updated);
            let _ = self
                .credential_store
                .store(&updated.keychain_service_key(), &self.form.password);
        } else {
            let _ = self.storage.save_connection(&profile);
            let _ = self
                .credential_store
                .store(&profile.keychain_service_key(), &self.form.password);
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
                    .text_color(rgb(0x888888))
                    .child("DATABASE NAVIGATOR"),
            )
            .child(
                div()
                    .id("add-connection-btn")
                    .cursor_pointer()
                    .text_sm()
                    .text_color(rgb(0x888888))
                    .hover(|s| s.text_color(rgb(0xeeeeee)))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.toggle_form(cx);
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
                    .text_color(rgb(0x555555))
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
                    .hover(|s| s.bg(rgb(0x2a2a2a)))
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
                            .text_color(rgb(0xcccccc))
                            .child(name),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("del-conn-{i}")))
                            .text_xs()
                            .text_color(rgb(0x555555))
                            .hover(|s| s.text_color(rgb(0xf14c4c)))
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

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .border_t_1()
            .border_color(rgb(0x333333))
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xcccccc))
                    .child("New Connection"),
            )
            // Host + Port row
            .child(self.render_form_field("Host", &self.form.host, "host-input", cx))
            .child(self.render_form_field("Port", &self.form.port, "port-input", cx))
            .child(self.render_form_field("Database", &self.form.database, "db-input", cx))
            .child(self.render_form_field("Username", &self.form.username, "user-input", cx))
            .child(self.render_form_field("Password", &self.form.password, "pass-input", cx))
            // Buttons
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(
                        div()
                            .id("cancel-btn")
                            .px_3()
                            .py_1()
                            .bg(rgb(0x333333))
                            .text_xs()
                            .text_color(rgb(0xcccccc))
                            .rounded_sm()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.show_form = false;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("save-btn")
                            .px_3()
                            .py_1()
                            .bg(rgb(0x4fc1ff))
                            .text_xs()
                            .text_color(rgb(0x1a1a1a))
                            .font_weight(FontWeight::SEMIBOLD)
                            .rounded_sm()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.save_connection(cx);
                            }))
                            .child("Save & Connect"),
                    ),
            )
    }

    fn render_form_field(
        &self,
        label: &str,
        _value: &str,
        id: &str,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x888888))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .id(SharedString::from(id.to_string()))
                    .h(px(24.0))
                    .px_2()
                    .bg(rgb(0x252525))
                    .border_1()
                    .border_color(rgb(0x3e3e3e))
                    .rounded_sm()
                    .text_xs()
                    .text_color(rgb(0xd4d4d4))
                    .child(_value.to_string()),
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
