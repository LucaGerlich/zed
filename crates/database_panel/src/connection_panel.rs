use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use editor::Editor;
use gpui::*;
use tokio::runtime::Runtime;
use ui::prelude::*;
use ui::{
    Button, ButtonStyle, ContextMenu, IconButton, IconName, IconSize, Label, LabelCommon,
    LabelSize, ListItem, ListItemSpacing, Tooltip, right_click_menu,
};
use workspace::Workspace;
use workspace::dock::{DockPosition, Panel, PanelEvent};

use pgblade_core::connection::{ConnectionProfile, Environment, SshAuth, SshConfig, SslMode};
use pgblade_core::driver::{DatabaseDriver, DatabaseSession};
use pgblade_core::schema::{SchemaTree, TableEntry, TableKind};
use pgblade_core::security::CredentialStore;
use pgblade_core::ssh::SshTunnel;
use pgblade_core::storage::StorageManager;
use pgblade_postgres::PostgresDriver;
use pgblade_security::KeychainStore;

use crate::ResultPanel;
use crate::SqlCompletionProvider;

/// Escape a string for use inside a SQL single-quoted literal.
/// Handles single quotes and backslashes.
pub(crate) fn escape_sql_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "''")
}

/// Escape a string for use in a LIKE/ILIKE pattern inside a SQL literal.
/// Escapes single quotes, backslashes, and LIKE wildcards (% and _).
pub(crate) fn escape_sql_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "''")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Escape a string for use inside a double-quoted SQL identifier.
#[allow(dead_code)]
pub(crate) fn escape_sql_identifier(s: &str) -> String {
    s.replace('"', "\"\"")
}

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
    form_ssl_mode: SslMode,
    // SSH tunnel form fields
    ssh_enabled: bool,
    ssh_host_editor: Entity<Editor>,
    ssh_port_editor: Entity<Editor>,
    ssh_username_editor: Entity<Editor>,
    ssh_key_editor: Entity<Editor>,
    ssh_auth: SshAuth,
    // Storage
    storage: StorageManager,
    credential_store: KeychainStore,
    // Database connection
    runtime: Arc<Runtime>,
    driver: PostgresDriver,
    session: Option<Arc<dyn DatabaseSession>>,
    connected_profile: Option<ConnectionProfile>,
    // Active SSH tunnel (kept alive while connected)
    ssh_tunnel: Option<SshTunnel>,
    // Schema tree state
    schema_tree: Option<SchemaTree>,
    expanded_nodes: HashSet<String>,
    // Workspace reference for cross-panel communication
    workspace: WeakEntity<Workspace>,
    // Error message to display in the panel
    error_message: Option<String>,
    // When editing an existing connection, holds the original ID
    editing_connection_id: Option<pgblade_core::connection::ConnectionId>,
    // Timestamp when the current connection was established
    connected_at: Option<std::time::Instant>,
    // SQL completion provider shared with editors for inline completions
    sql_completion_provider: Rc<SqlCompletionProvider>,
}

impl ConnectionPanel {
    pub fn new(
        workspace: WeakEntity<Workspace>,
        sql_completion_provider: Rc<SqlCompletionProvider>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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

        let ssh_host_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("ssh.example.com", window, cx);
            editor
        });

        let ssh_port_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("22", window, cx);
            editor.set_text("22", window, cx);
            editor
        });

        let ssh_username_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("username", window, cx);
            editor
        });

        let ssh_key_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("~/.ssh/id_rsa", window, cx);
            editor
        });

        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime"),
        );
        let driver = PostgresDriver::new(runtime.clone());

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
            form_ssl_mode: SslMode::Disable,
            ssh_enabled: false,
            ssh_host_editor,
            ssh_port_editor,
            ssh_username_editor,
            ssh_key_editor,
            ssh_auth: SshAuth::Agent,
            storage,
            credential_store: KeychainStore::new(),
            runtime,
            driver,
            session: None,
            connected_profile: None,
            ssh_tunnel: None,
            schema_tree: None,
            expanded_nodes: HashSet::new(),
            workspace,
            error_message: None,
            editing_connection_id: None,
            connected_at: None,
            sql_completion_provider,
        }
    }

    fn toggle_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_form = !self.show_form;
        if self.show_form {
            // Clear editing state for a fresh new connection form
            self.editing_connection_id = None;
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
            self.form_ssl_mode = SslMode::Disable;

            // Reset SSH fields
            self.ssh_enabled = false;
            self.ssh_host_editor.update(cx, |editor, cx| {
                editor.set_text("", window, cx);
            });
            self.ssh_port_editor.update(cx, |editor, cx| {
                editor.set_text("22", window, cx);
            });
            self.ssh_username_editor.update(cx, |editor, cx| {
                editor.set_text("", window, cx);
            });
            self.ssh_key_editor.update(cx, |editor, cx| {
                editor.set_text("", window, cx);
            });
            self.ssh_auth = SshAuth::Agent;
        }
        cx.notify();
    }

    fn save_connection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host_editor.read(cx).text(cx);
        let port_str = self.port_editor.read(cx).text(cx);
        let database = self.database_editor.read(cx).text(cx);
        let username = self.username_editor.read(cx).text(cx);
        let password = self.password_editor.read(cx).text(cx);

        let ssh = if self.ssh_enabled {
            let key_path = self.ssh_key_editor.read(cx).text(cx);
            let auth = match &self.ssh_auth {
                SshAuth::KeyFile { .. } if !key_path.trim().is_empty() => SshAuth::KeyFile {
                    path: key_path.trim().to_string(),
                },
                _ => SshAuth::Agent,
            };
            SshConfig {
                enabled: true,
                host: self.ssh_host_editor.read(cx).text(cx),
                port: self.ssh_port_editor.read(cx).text(cx).parse().unwrap_or(22),
                username: self.ssh_username_editor.read(cx).text(cx),
                auth,
            }
        } else {
            SshConfig::default()
        };

        // Reuse the editing ID if editing an existing connection, otherwise generate new
        let connection_id = self.editing_connection_id.take().unwrap_or_default();

        let profile = ConnectionProfile {
            id: connection_id,
            name: format!("{}@{}", database, host),
            host,
            port: port_str.parse().unwrap_or(5432),
            database,
            username,
            environment: self.form_environment,
            ssl_mode: self.form_ssl_mode,
            read_only_default: self.form_environment.is_production(),
            ssh,
        };

        // Check for duplicates
        if let Ok(Some(existing_id)) = self.storage.find_duplicate(&profile) {
            let mut updated = profile.clone();
            updated.id = existing_id;
            let _ = self.storage.save_connection(&updated);
            let _ = self.storage.save_password(&updated.id, &password);
            let _ = self
                .credential_store
                .store(&updated.keychain_service_key(), &password);
        } else {
            let _ = self.storage.save_connection(&profile);
            let _ = self.storage.save_password(&profile.id, &password);
            let _ = self
                .credential_store
                .store(&profile.keychain_service_key(), &password);
        }

        self.saved_connections = self.storage.load_connections().unwrap_or_default();
        self.show_form = false;
        cx.notify();

        // Connect to the database
        self.connect(profile, password, cx);
    }

    fn test_connection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host_editor.read(cx).text(cx);
        let port_str = self.port_editor.read(cx).text(cx);
        let database = self.database_editor.read(cx).text(cx);
        let username = self.username_editor.read(cx).text(cx);
        let password = self.password_editor.read(cx).text(cx);

        let profile = ConnectionProfile {
            id: pgblade_core::connection::ConnectionId::new(),
            name: "test".to_string(),
            host,
            port: port_str.parse().unwrap_or(5432),
            database,
            username,
            environment: self.form_environment,
            ssl_mode: self.form_ssl_mode,
            read_only_default: false,
            ssh: SshConfig::default(),
        };

        let driver = self.driver.clone();
        let runtime = self.runtime.clone();

        self.error_message = Some("Testing connection...".to_string());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { driver.connect(&profile, &password).await })
                .await;

            this.update(cx, |panel, cx| {
                match result {
                    Ok(Ok(session)) => {
                        let version = session.server_version().to_string();
                        panel.error_message =
                            Some(format!("Connection successful! Server: {version}"));
                        // Don't store the session -- just testing
                        drop(session);
                    }
                    Ok(Err(e)) => {
                        panel.error_message = Some(format!("Connection failed: {e}"));
                    }
                    Err(e) => {
                        panel.error_message = Some(format!("Runtime error: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn connect_saved(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.saved_connections.get(index).cloned() else {
            return;
        };

        // Try SQLite first (reliable), then keychain as fallback
        let password = self
            .storage
            .load_password(&profile.id)
            .ok()
            .flatten()
            .or_else(|| {
                let key = profile.keychain_service_key();
                self.credential_store.retrieve(&key).ok().flatten()
            })
            .unwrap_or_default();

        if password.is_empty() {
            tracing::warn!("No password found for connection {}", profile.name);
            self.error_message =
                Some("No password found. Please edit the connection and re-enter.".to_string());
            cx.notify();
            return;
        }

        self.connect(profile, password, cx);
    }

    fn prefill_form_from_profile(
        &mut self,
        profile: &ConnectionProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing_connection_id = Some(profile.id);
        let host = profile.host.clone();
        let port = profile.port.to_string();
        let database = profile.database.clone();
        let username = profile.username.clone();
        self.host_editor
            .update(cx, |e, cx| e.set_text(host, window, cx));
        self.port_editor
            .update(cx, |e, cx| e.set_text(port, window, cx));
        self.database_editor
            .update(cx, |e, cx| e.set_text(database, window, cx));
        self.username_editor
            .update(cx, |e, cx| e.set_text(username, window, cx));
        self.password_editor
            .update(cx, |e, cx| e.set_text("", window, cx));
        self.form_environment = profile.environment;
        self.form_ssl_mode = profile.ssl_mode;

        if profile.ssh.enabled {
            self.ssh_enabled = true;
            let ssh_host = profile.ssh.host.clone();
            let ssh_port = profile.ssh.port.to_string();
            let ssh_username = profile.ssh.username.clone();
            self.ssh_host_editor
                .update(cx, |e, cx| e.set_text(ssh_host, window, cx));
            self.ssh_port_editor
                .update(cx, |e, cx| e.set_text(ssh_port, window, cx));
            self.ssh_username_editor
                .update(cx, |e, cx| e.set_text(ssh_username, window, cx));
            self.ssh_auth = profile.ssh.auth.clone();
            if let SshAuth::KeyFile { ref path } = profile.ssh.auth {
                let key_path = path.clone();
                self.ssh_key_editor
                    .update(cx, |e, cx| e.set_text(key_path, window, cx));
            }
        } else {
            self.ssh_enabled = false;
        }
    }

    pub fn connect(
        &mut self,
        mut profile: ConnectionProfile,
        password: String,
        cx: &mut Context<Self>,
    ) {
        // Clear any previous error
        self.error_message = None;

        // Establish SSH tunnel if configured
        if profile.ssh.enabled {
            let original_host = profile.host.clone();
            let original_port = profile.port;

            match SshTunnel::start(&profile.ssh, &original_host, original_port) {
                Ok(tunnel) => {
                    tracing::info!(
                        "SSH tunnel established: localhost:{} -> {}:{}",
                        tunnel.local_port(),
                        original_host,
                        original_port
                    );
                    profile.host = "127.0.0.1".to_string();
                    profile.port = tunnel.local_port();
                    self.ssh_tunnel = Some(tunnel);
                }
                Err(e) => {
                    tracing::error!("SSH tunnel failed: {e}");
                    self.error_message = Some(format!("SSH tunnel failed: {e}"));
                    cx.notify();
                    return;
                }
            }
        }

        let driver = self.driver.clone();
        let runtime = self.runtime.clone();
        let connect_profile = profile.clone();
        let password_for_save = password.clone();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { driver.connect(&connect_profile, &password).await })
                .await;

            match result {
                Ok(Ok(session)) => {
                    let session: Arc<dyn DatabaseSession> = Arc::from(session);
                    this.update(cx, |panel, cx| {
                        panel.session = Some(session);
                        panel.connected_profile = Some(profile.clone());
                        panel.connected_at = Some(std::time::Instant::now());
                        // Ensure password is stored in SQLite for next time
                        let _ = panel.storage.save_password(&profile.id, &password_for_save);
                        tracing::info!("database connection established");
                        cx.emit(PanelEvent::Activate);
                        cx.notify();
                        panel.fetch_schema(cx);
                        panel.start_health_check(cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("connection failed: {e}");
                    this.update(cx, |panel, cx| {
                        panel.ssh_tunnel = None;
                        panel.error_message = Some(format!("Connection failed: {e}"));
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("runtime error: {e}");
                    this.update(cx, |panel, cx| {
                        panel.ssh_tunnel = None;
                        panel.error_message = Some(format!("Runtime error: {e}"));
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn fetch_schema(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let runtime = self.runtime.clone();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move {
                    let schemas = session.list_schemas().await?;
                    let mut entries = Vec::new();

                    for schema in &schemas {
                        let tables_raw = session.list_tables(&schema.name).await?;
                        let mut tables = Vec::new();
                        let mut views = Vec::new();
                        let mut mat_views = Vec::new();

                        for table in &tables_raw {
                            let columns = session.list_columns(&schema.name, &table.name).await?;
                            let constraints =
                                session.list_constraints(&schema.name, &table.name).await?;
                            let foreign_keys =
                                session.list_foreign_keys(&schema.name, &table.name).await?;
                            let indexes = session.list_indexes(&schema.name, &table.name).await?;
                            let triggers = session.list_triggers(&schema.name, &table.name).await?;

                            let entry = TableEntry {
                                info: table.clone(),
                                columns,
                                constraints,
                                foreign_keys,
                                indexes,
                                triggers,
                            };

                            match table.kind {
                                TableKind::View => views.push(entry),
                                TableKind::MaterializedView => mat_views.push(entry),
                                TableKind::Table => tables.push(entry),
                            }
                        }

                        let functions = session.list_functions(&schema.name).await?;
                        let sequences = session.list_sequences(&schema.name).await?;

                        entries.push(pgblade_core::schema::SchemaEntry {
                            info: schema.clone(),
                            tables,
                            views,
                            materialized_views: mat_views,
                            functions,
                            sequences,
                        });
                    }

                    Ok::<_, pgblade_core::error::QueryError>(SchemaTree { schemas: entries })
                })
                .await;

            match result {
                Ok(Ok(tree)) => {
                    this.update(cx, |panel, cx| {
                        // Auto-expand default nodes
                        if let Some(profile) = &panel.connected_profile {
                            panel
                                .expanded_nodes
                                .insert(format!("conn:{}", profile.name));
                            panel
                                .expanded_nodes
                                .insert(format!("db:{}", profile.database));
                            for schema in &tree.schemas {
                                if schema.info.is_default {
                                    panel
                                        .expanded_nodes
                                        .insert(format!("schema:{}", schema.info.name));
                                    panel
                                        .expanded_nodes
                                        .insert(format!("tables:{}", schema.info.name));
                                }
                            }
                        }
                        let items = crate::sql_completion::build_completion_items(&tree);
                        panel.sql_completion_provider.set_items(items);
                        panel.schema_tree = Some(tree);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("schema introspection failed: {e}");
                }
                Err(e) => {
                    tracing::error!("runtime error during schema fetch: {e}");
                }
            }
        })
        .detach();
    }

    /// Start a background health check that pings the database every 30 seconds.
    /// If the connection is lost, the panel resets to disconnected state.
    fn start_health_check(&mut self, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(30))
                    .await;

                // Get the current session from the panel each iteration
                let session = this
                    .update(cx, |panel, _cx| panel.session.clone())
                    .ok()
                    .flatten();

                let Some(session) = session else {
                    // Panel was disconnected or dropped, stop health checking
                    break;
                };

                let rt = runtime.clone();
                let result = rt
                    .spawn(async move { session.execute("SELECT 1").await })
                    .await;

                match result {
                    Ok(Ok(_)) => {
                        // Connection is healthy
                    }
                    _ => {
                        // Connection dropped
                        tracing::warn!("health check failed, marking connection as lost");
                        this.update(cx, |panel, cx| {
                            panel.error_message = Some(
                                "Connection lost. Click a saved connection to reconnect."
                                    .to_string(),
                            );
                            panel.disconnect(cx);
                        })
                        .ok();
                        break;
                    }
                }
            }
        })
        .detach();
    }

    /// Returns true if there are saved connections in storage.
    pub fn has_saved_connections(&self) -> bool {
        !self.saved_connections.is_empty()
    }

    /// Returns the active database session, if connected.
    pub fn session(&self) -> Option<Arc<dyn DatabaseSession>> {
        self.session.clone()
    }

    /// Returns the introspected schema tree, if available.
    pub fn schema_tree(&self) -> Option<&SchemaTree> {
        self.schema_tree.as_ref()
    }

    /// Returns the tokio runtime used for database operations.
    pub fn runtime(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    /// Save a query to the history store.
    pub fn save_to_history(&self, sql: &str) {
        let database = self
            .connected_profile
            .as_ref()
            .map(|p| p.database.as_str())
            .unwrap_or("");
        let record = pgblade_core::query::QueryRecord {
            id: pgblade_core::query::QueryId::new(),
            sql: sql.to_string(),
            classification: pgblade_core::classifier::classify_sql(sql),
            executed_at: chrono::Utc::now(),
            duration_ms: None,
            row_count: None,
            error: None,
            database: database.to_string(),
        };
        let _ = self.storage.save_query(&record);
    }

    /// Load recent query history records.
    pub fn load_history(&self) -> Vec<pgblade_core::query::QueryRecord> {
        self.storage.load_recent_history(100).unwrap_or_default()
    }

    /// Save the current SQL as a bookmark using the first line (up to 50 chars) as the name.
    pub fn save_bookmark(&self, sql: &str) {
        let name: String = sql
            .lines()
            .next()
            .unwrap_or("Query")
            .chars()
            .take(50)
            .collect();
        let _ = self.storage.save_bookmark(&name, sql);
    }

    /// Load all saved bookmarks.
    pub fn load_bookmarks(&self) -> Vec<(String, String, String, String)> {
        self.storage.load_bookmarks().unwrap_or_default()
    }

    /// Disconnect from the current database session and reset state.
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        self.connected_profile = None;
        self.connected_at = None;
        self.ssh_tunnel = None;
        self.schema_tree = None;
        self.expanded_nodes.clear();
        self.error_message = None;
        self.show_form = false;
        self.ssh_enabled = false;
        self.sql_completion_provider.set_items(Vec::new());
        // Reload saved connections so the list is fresh
        self.saved_connections = self.storage.load_connections().unwrap_or_default();
        tracing::info!(
            "disconnected, {} saved connections available",
            self.saved_connections.len()
        );
        cx.notify();
    }

    /// Returns the currently connected profile, if any.
    pub fn connected_profile(&self) -> Option<&ConnectionProfile> {
        self.connected_profile.as_ref()
    }

    /// Returns the environment of the currently connected profile, if any.
    pub fn connection_environment(&self) -> Option<Environment> {
        self.connected_profile.as_ref().map(|p| p.environment)
    }

    /// Returns a formatted status bar string showing connection info and uptime.
    pub fn status_bar_text(&self) -> Option<String> {
        let profile = self.connected_profile.as_ref()?;
        let uptime = self
            .connected_at
            .map(|t| {
                let secs = t.elapsed().as_secs();
                if secs < 60 {
                    format!("{secs}s")
                } else if secs < 3600 {
                    format!("{}m", secs / 60)
                } else {
                    format!("{}h", secs / 3600)
                }
            })
            .unwrap_or_default();
        Some(format!("{}@{} ({uptime})", profile.database, profile.host))
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

    fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded_nodes.contains(node_id)
    }

    /// Preview table data by executing SELECT * FROM ... (result panel handles LIMIT).
    fn preview_table(
        &self,
        schema: &str,
        table: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let runtime = self.runtime.clone();
        let sql = format!("SELECT * FROM \"{schema}\".\"{table}\"");
        let source_table = Some((schema.to_string(), table.to_string()));

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.open_panel::<ResultPanel>(window, cx);
                if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
                    result_panel.update(cx, |panel, cx| {
                        panel.execute_query_with_source(sql, session, runtime, source_table, cx);
                    });
                }
            });
        }
    }

    /// Generate a SELECT statement for a table and insert it into the active editor.
    fn generate_select(
        &self,
        schema: &str,
        table: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sql = format!("SELECT *\nFROM \"{schema}\".\"{table}\"\nLIMIT 100;\n");

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                if let Some(active_item) = workspace.active_item(cx)
                    && let Some(editor) = active_item.act_as::<Editor>(cx)
                {
                    editor.update(cx, |editor, cx| {
                        let text = editor.text(cx);
                        let insert_text = if text.is_empty() {
                            sql
                        } else {
                            format!("\n\n{sql}")
                        };
                        editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
                        editor.insert(&insert_text, window, cx);
                    });
                }
            });
        }
    }

    /// Generate an INSERT statement template for a table and insert it into the active editor.
    fn generate_insert(
        &self,
        schema: &str,
        table: &str,
        columns: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cols = columns.join(", ");
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${i}")).collect();
        let vals = placeholders.join(", ");
        let sql = format!("INSERT INTO \"{schema}\".\"{table}\" ({cols})\nVALUES ({vals});");
        self.insert_sql_into_editor(&sql, window, cx);
    }

    /// Generate an UPDATE statement template for a table and insert it into the active editor.
    fn generate_update(
        &self,
        schema: &str,
        table: &str,
        columns: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sets: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| format!("    {c} = ${}", i + 1))
            .collect();
        let sql = format!(
            "UPDATE \"{schema}\".\"{table}\"\nSET\n{}\nWHERE /* condition */;",
            sets.join(",\n")
        );
        self.insert_sql_into_editor(&sql, window, cx);
    }

    /// Generate a CREATE INDEX template for a table and insert it into the active editor.
    fn generate_create_index(
        &self,
        schema: &str,
        table: &str,
        columns: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let first_col = columns
            .first()
            .map(|c| c.trim_matches('"'))
            .unwrap_or("column_name");
        let safe_col = first_col.replace('"', "");
        let sql = format!(
            "CREATE INDEX IF NOT EXISTS idx_{table}_{safe_col}\n    ON \"{schema}\".\"{table}\" ({first_col});"
        );
        self.insert_sql_into_editor(&sql, window, cx);
    }

    /// Insert SQL text into the active editor at the end.
    fn insert_sql_into_editor(&self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace.upgrade() {
            let sql = sql.to_string();
            workspace.update(cx, |workspace, cx| {
                if let Some(active_item) = workspace.active_item(cx)
                    && let Some(editor) = active_item.act_as::<Editor>(cx)
                {
                    editor.update(cx, |editor, cx| {
                        let text = editor.text(cx);
                        let prefix = if text.is_empty() { "" } else { "\n\n" };
                        editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
                        editor.insert(&format!("{prefix}{sql}"), window, cx);
                    });
                }
            });
        }
    }

    /// Execute a SQL statement in the result panel (shared helper for table actions).
    fn execute_in_result_panel(&self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let runtime = self.runtime.clone();
        let sql = sql.to_string();

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.open_panel::<ResultPanel>(window, cx);
                if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
                    result_panel.update(cx, |panel, cx| {
                        panel.execute_query(sql, session, runtime, cx);
                    });
                }
            });
        }
    }

    /// Run SELECT COUNT(*) for a table and show the result.
    fn count_table(&self, schema: &str, table: &str, window: &mut Window, cx: &mut Context<Self>) {
        let sql = format!("SELECT COUNT(*) AS row_count FROM \"{schema}\".\"{table}\"");
        self.execute_in_result_panel(&sql, window, cx);
    }

    /// Run TRUNCATE TABLE for a table.
    fn truncate_table(
        &self,
        schema: &str,
        table: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sql = format!("TRUNCATE TABLE \"{schema}\".\"{table}\"");
        self.execute_in_result_panel(&sql, window, cx);
    }

    /// Run ANALYZE for a table (VACUUM cannot run inside a transaction).
    fn vacuum_table(&self, schema: &str, table: &str, window: &mut Window, cx: &mut Context<Self>) {
        let sql = format!("ANALYZE \"{schema}\".\"{table}\"");
        self.execute_in_result_panel(&sql, window, cx);
    }

    /// Refresh a materialized view.
    fn refresh_materialized_view(
        &self,
        schema: &str,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sql = format!("REFRESH MATERIALIZED VIEW \"{schema}\".\"{name}\"");
        self.execute_in_result_panel(&sql, window, cx);
    }

    /// Show DDL for a table by finding it in the schema tree and generating the statement.
    fn show_ddl(&self, schema: &str, table: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tree) = &self.schema_tree else {
            return;
        };

        // Find the table entry in the schema tree
        let table_entry = tree
            .schemas
            .iter()
            .find(|s| s.info.name == schema)
            .and_then(|s| {
                s.tables
                    .iter()
                    .chain(s.views.iter())
                    .chain(s.materialized_views.iter())
                    .find(|t| t.info.name == table)
            });

        let Some(entry) = table_entry else {
            return;
        };

        let ddl = Self::generate_ddl(entry);

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace.open_panel::<ResultPanel>(window, cx);
                if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
                    result_panel.update(cx, |panel, cx| {
                        panel.show_ddl(ddl, cx);
                    });
                }
            });
        }
    }

    /// Show the SQL definition of a view or materialized view.
    fn show_view_definition(&self, view_name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let safe_name = escape_sql_string(view_name);
        let sql = format!(
            "SELECT definition FROM pg_views WHERE viewname = '{safe_name}' \
             UNION ALL \
             SELECT definition FROM pg_matviews WHERE matviewname = '{safe_name}'"
        );
        self.execute_in_result_panel(&sql, window, cx);
    }

    /// Show the source code of a stored function as DDL in the result panel.
    fn show_function_source(
        &self,
        func_name: &str,
        schema: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let runtime = self.runtime.clone();
        let safe_name = escape_sql_string(func_name);
        let safe_schema = escape_sql_string(schema);
        let sql = format!(
            "SELECT pg_get_functiondef(p.oid) AS definition \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             WHERE p.proname = '{safe_name}' AND n.nspname = '{safe_schema}'"
        );
        let func_display = format!("{schema}.{func_name}");

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql).await })
                .await;

            match result {
                Ok(Ok(result_set)) => {
                    let definition = result_set
                        .rows
                        .first()
                        .and_then(|row| row.first())
                        .map(|cell| cell.display())
                        .unwrap_or_else(|| {
                            format!("-- Could not fetch definition for {func_display}")
                        });

                    let ddl = format!(
                        "-- Function: {func_display}\n\
                         -- Edit below and execute with Cmd+Enter to update\n\n\
                         {definition}"
                    );

                    this.update(&mut *cx, |panel, cx| {
                        if let Some(workspace) = panel.workspace.upgrade() {
                            workspace.update(cx, |workspace, cx| {
                                if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
                                    result_panel.update(cx, |rpanel, cx| {
                                        rpanel.show_ddl(ddl, cx);
                                    });
                                }
                            });
                        }
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("failed to fetch function source: {e}");
                }
                Err(e) => {
                    tracing::error!("runtime error fetching function source: {e}");
                }
            }
        })
        .detach();
    }

    /// Show the source code of a trigger as DDL in the result panel.
    fn show_trigger_source(
        &self,
        trigger_name: &str,
        table_name: &str,
        schema: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let runtime = self.runtime.clone();
        let safe_trigger = escape_sql_string(trigger_name);
        let safe_table = escape_sql_string(table_name);
        let safe_schema = escape_sql_string(schema);
        let sql = format!(
            "SELECT pg_get_triggerdef(t.oid, true) AS definition \
             FROM pg_trigger t \
             JOIN pg_class c ON c.oid = t.tgrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE t.tgname = '{safe_trigger}' \
             AND c.relname = '{safe_table}' \
             AND n.nspname = '{safe_schema}'"
        );
        let trigger_display = format!("{schema}.{table_name}.{trigger_name}");

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql).await })
                .await;

            match result {
                Ok(Ok(result_set)) => {
                    let definition = result_set
                        .rows
                        .first()
                        .and_then(|row| row.first())
                        .map(|cell| cell.display())
                        .unwrap_or_else(|| {
                            format!("-- Could not fetch definition for {trigger_display}")
                        });

                    let ddl = format!(
                        "-- Trigger: {trigger_display}\n\
                         -- Edit below and execute with Cmd+Enter to update\n\n\
                         {definition}"
                    );

                    this.update(&mut *cx, |panel, cx| {
                        if let Some(workspace) = panel.workspace.upgrade() {
                            workspace.update(cx, |workspace, cx| {
                                if let Some(result_panel) = workspace.panel::<ResultPanel>(cx) {
                                    result_panel.update(cx, |rpanel, cx| {
                                        rpanel.show_ddl(ddl, cx);
                                    });
                                }
                            });
                        }
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("failed to fetch trigger source: {e}");
                }
                Err(e) => {
                    tracing::error!("runtime error fetching trigger source: {e}");
                }
            }
        })
        .detach();
    }

    /// Render a trigger row with a SRC button that fetches the trigger definition.
    #[allow(clippy::too_many_arguments)]
    fn render_trigger_row(
        &self,
        node_id: &str,
        label: &str,
        trigger_name: &str,
        table_name: &str,
        schema_name: &str,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = SharedString::from(format!("tree-{node_id}"));
        let trigger_for_src = trigger_name.to_string();
        let table_for_src = table_name.to_string();
        let schema_for_src = schema_name.to_string();

        ListItem::new(id)
            .indent_level(depth)
            .indent_step_size(px(12.))
            .spacing(ListItemSpacing::ExtraDense)
            .end_slot_on_hover(
                IconButton::new(SharedString::from(format!("src-{node_id}")), IconName::Code)
                    .icon_size(IconSize::XSmall)
                    .icon_color(Color::Muted)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("View Source"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_trigger_source(
                            &trigger_for_src,
                            &table_for_src,
                            &schema_for_src,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::DatabaseZap)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(Label::new(label.to_string()).size(LabelSize::Small)),
            )
            .into_any_element()
    }

    /// Generate CREATE TABLE DDL from introspected schema information.
    pub fn generate_ddl(table: &TableEntry) -> String {
        let kind_keyword = match table.info.kind {
            TableKind::View => "VIEW",
            TableKind::MaterializedView => "MATERIALIZED VIEW",
            TableKind::Table => "TABLE",
        };

        if matches!(
            table.info.kind,
            TableKind::View | TableKind::MaterializedView
        ) {
            // For views we don't have the SELECT definition, show a placeholder
            return format!(
                "-- DDL for {} \"{}\".\"{}\" (view definition not available via introspection)\n\
                 CREATE {} \"{}\".\"{}\" AS\n  SELECT ...;\n",
                kind_keyword,
                table.info.schema,
                table.info.name,
                kind_keyword,
                table.info.schema,
                table.info.name,
            );
        }

        let mut ddl = format!(
            "CREATE TABLE \"{}\".\"{}\" (\n",
            table.info.schema, table.info.name
        );

        let col_count = table.columns.len();
        for (i, col) in table.columns.iter().enumerate() {
            let not_null = if col.nullable { "" } else { " NOT NULL" };
            let default = col
                .default_value
                .as_ref()
                .map(|d| format!(" DEFAULT {d}"))
                .unwrap_or_default();
            let trailing_comma = if i + 1 < col_count { "," } else { "" };
            ddl.push_str(&format!(
                "    \"{}\" {}{}{}{}\n",
                col.name, col.data_type, not_null, default, trailing_comma
            ));
        }

        // Primary key constraint
        let pk_cols: Vec<&str> = table
            .columns
            .iter()
            .filter(|c| c.is_primary_key)
            .map(|c| c.name.as_str())
            .collect();
        if !pk_cols.is_empty() {
            let pk_list = pk_cols
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");
            // Add comma after last column if we have a PK
            if col_count > 0 {
                // Replace last newline to add comma
                if ddl.ends_with('\n') {
                    ddl.pop();
                    // Find and replace the last non-comma character before newline
                    if !ddl.ends_with(',') {
                        ddl.push(',');
                    }
                    ddl.push('\n');
                }
            }
            ddl.push_str(&format!(
                "    CONSTRAINT \"{}_pkey\" PRIMARY KEY ({})\n",
                table.info.name, pk_list
            ));
        }

        ddl.push_str(");\n");

        // Indexes (non-PK)
        for idx in &table.indexes {
            let unique = if idx.is_unique { "UNIQUE " } else { "" };
            let cols = idx
                .columns
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");
            ddl.push_str(&format!(
                "\nCREATE {}INDEX \"{}\" ON \"{}\".\"{}\" USING {} ({});",
                unique, idx.name, table.info.schema, table.info.name, idx.index_type, cols
            ));
        }

        if !table.indexes.is_empty() {
            ddl.push('\n');
        }

        ddl
    }

    fn render_tree_row(
        &self,
        node_id: &str,
        label: &str,
        depth: usize,
        has_children: bool,
        icon: Option<IconName>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_expanded = self.expanded_nodes.contains(node_id);
        let id = SharedString::from(format!("tree-{node_id}"));
        let node_id_owned = node_id.to_string();

        ListItem::new(id)
            .indent_level(depth)
            .indent_step_size(px(12.))
            .spacing(ListItemSpacing::ExtraDense)
            .toggle(has_children.then_some(is_expanded))
            .on_toggle(cx.listener({
                let node_id = node_id_owned.clone();
                move |this, _, _window, cx| {
                    if this.expanded_nodes.contains(&node_id) {
                        this.expanded_nodes.remove(&node_id);
                    } else {
                        this.expanded_nodes.insert(node_id.clone());
                    }
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _window, cx| {
                if has_children {
                    if this.expanded_nodes.contains(&node_id_owned) {
                        this.expanded_nodes.remove(&node_id_owned);
                    } else {
                        this.expanded_nodes.insert(node_id_owned.clone());
                    }
                    cx.notify();
                }
            }))
            .child(
                h_flex()
                    .gap_1()
                    .children(icon.map(|i| Icon::new(i).size(IconSize::XSmall).color(Color::Muted)))
                    .child(Label::new(label.to_string()).size(LabelSize::Small)),
            )
            .into_any_element()
    }

    /// Render a function row with a SRC button that fetches the function source code.
    fn render_function_row(
        &self,
        node_id: &str,
        label: &str,
        func_name: &str,
        schema_name: &str,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = SharedString::from(format!("tree-{node_id}"));
        let func_for_src = func_name.to_string();
        let schema_for_src = schema_name.to_string();

        ListItem::new(id)
            .indent_level(depth)
            .indent_step_size(px(12.))
            .spacing(ListItemSpacing::ExtraDense)
            .end_slot_on_hover(
                IconButton::new(SharedString::from(format!("src-{node_id}")), IconName::Code)
                    .icon_size(IconSize::XSmall)
                    .icon_color(Color::Muted)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("View Source"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_function_source(&func_for_src, &schema_for_src, window, cx);
                    })),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::Code)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(Label::new(label.to_string()).size(LabelSize::Small)),
            )
            .into_any_element()
    }

    /// Format a number with K/M suffixes for display.
    fn format_number(n: i64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }

    /// Render a table row that expands on click AND triggers data preview.
    /// Also includes action buttons visible on hover.
    /// For views/materialized views, a DEF button is shown to fetch the view definition.
    #[allow(clippy::too_many_arguments)]
    fn render_table_row(
        &self,
        node_id: &str,
        table_name: &str,
        schema_name: &str,
        columns: &[String],
        kind: TableKind,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_expanded = self.expanded_nodes.contains(node_id);
        let id = SharedString::from(format!("tree-{node_id}"));
        let node_id_owned = node_id.to_string();
        let schema_owned = schema_name.to_string();
        // Extract the raw table name (strip row estimate suffix like " (~1.2K)")
        let table_owned = table_name
            .find(" (~")
            .map(|i| &table_name[..i])
            .unwrap_or(table_name)
            .to_string();

        // Build hover action buttons
        let schema_for_sql = schema_name.to_string();
        let table_for_sql = table_name.to_string();
        let schema_for_ins = schema_name.to_string();
        let table_for_ins = table_name.to_string();
        let cols_for_ins = columns.to_vec();
        let schema_for_upd = schema_name.to_string();
        let table_for_upd = table_name.to_string();
        let cols_for_upd = columns.to_vec();
        let schema_for_idx = schema_name.to_string();
        let table_for_idx = table_name.to_string();
        let cols_for_idx = columns.to_vec();
        let schema_for_ddl = schema_name.to_string();
        let table_for_ddl = table_name.to_string();
        let schema_for_cnt = schema_name.to_string();
        let table_for_cnt = table_name.to_string();
        let schema_for_trunc = schema_name.to_string();
        let table_for_trunc = table_name.to_string();
        let schema_for_vac = schema_name.to_string();
        let table_for_vac = table_name.to_string();
        let table_for_def = table_name.to_string();
        let schema_for_ref = schema_name.to_string();
        let table_for_ref = table_name.to_string();
        let show_def_button = matches!(kind, TableKind::View | TableKind::MaterializedView);
        let show_ref_button = matches!(kind, TableKind::MaterializedView);

        let icon = match kind {
            TableKind::View | TableKind::MaterializedView => IconName::Eye,
            TableKind::Table => IconName::ListTree,
        };

        let mut hover_actions = h_flex().gap_0p5();

        hover_actions = hover_actions
            .child(
                Button::new(SharedString::from(format!("sql-{node_id}")), "SEL")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_select(&schema_for_sql, &table_for_sql, window, cx);
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("ins-{node_id}")), "INS")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_insert(
                            &schema_for_ins,
                            &table_for_ins,
                            &cols_for_ins,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("upd-{node_id}")), "UPD")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_update(
                            &schema_for_upd,
                            &table_for_upd,
                            &cols_for_upd,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("idx-{node_id}")), "IDX")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_create_index(
                            &schema_for_idx,
                            &table_for_idx,
                            &cols_for_idx,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("ddl-{node_id}")), "DDL")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_ddl(&schema_for_ddl, &table_for_ddl, window, cx);
                    })),
            );

        if show_def_button {
            hover_actions = hover_actions.child(
                Button::new(SharedString::from(format!("def-{node_id}")), "DEF")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_view_definition(&table_for_def, window, cx);
                    })),
            );
        }

        if show_ref_button {
            hover_actions = hover_actions.child(
                Button::new(SharedString::from(format!("ref-{node_id}")), "REF")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.refresh_materialized_view(&schema_for_ref, &table_for_ref, window, cx);
                    })),
            );
        }

        hover_actions = hover_actions
            .child(
                Button::new(SharedString::from(format!("cnt-{node_id}")), "CNT")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.count_table(&schema_for_cnt, &table_for_cnt, window, cx);
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("trunc-{node_id}")), "TRC")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.truncate_table(&schema_for_trunc, &table_for_trunc, window, cx);
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("vac-{node_id}")), "VAC")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.vacuum_table(&schema_for_vac, &table_for_vac, window, cx);
                    })),
            );

        ListItem::new(id)
            .indent_level(depth)
            .indent_step_size(px(12.))
            .spacing(ListItemSpacing::ExtraDense)
            .toggle(Some(is_expanded))
            .on_toggle(cx.listener({
                let node_id = node_id_owned.clone();
                move |this, _, _window, cx| {
                    if this.expanded_nodes.contains(&node_id) {
                        this.expanded_nodes.remove(&node_id);
                    } else {
                        this.expanded_nodes.insert(node_id.clone());
                    }
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() >= 2 {
                    // Double-click: preview table data
                    this.preview_table(&schema_owned, &table_owned, window, cx);
                } else {
                    // Single-click: expand/collapse
                    if this.expanded_nodes.contains(&node_id_owned) {
                        this.expanded_nodes.remove(&node_id_owned);
                    } else {
                        this.expanded_nodes.insert(node_id_owned.clone());
                    }
                    cx.notify();
                }
            }))
            .end_slot_on_hover(hover_actions)
            .child(
                h_flex()
                    .gap_1()
                    .child(Icon::new(icon).size(IconSize::XSmall).color(Color::Muted))
                    .child(Label::new(table_name.to_string()).size(LabelSize::Small)),
            )
            .into_any_element()
    }

    fn render_table_entry(
        &self,
        table: &TableEntry,
        schema_name: &str,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let qualified = format!("{}.{}", schema_name, table.info.name);
        let table_id = format!("table:{qualified}");
        let col_names: Vec<String> = table
            .columns
            .iter()
            .map(|c| format!("\"{}\"", c.name))
            .collect();
        let row_estimate = table
            .info
            .row_estimate
            .map(|r| format!(" (~{})", Self::format_number(r)))
            .unwrap_or_default();
        let label = format!("{}{}", table.info.name, row_estimate);
        rows.push(self.render_table_row(
            &table_id,
            &label,
            schema_name,
            &col_names,
            table.info.kind,
            depth,
            cx,
        ));

        if self.is_expanded(&table_id) {
            // Columns
            let cols_id = format!("cols:{qualified}");
            rows.push(self.render_tree_row(
                &cols_id,
                &format!("Columns ({})", table.columns.len()),
                depth + 1,
                !table.columns.is_empty(),
                Some(IconName::ListTree),
                cx,
            ));
            if self.is_expanded(&cols_id) {
                for col in &table.columns {
                    let col_icon = if col.is_primary_key {
                        Some(IconName::LockOutlined)
                    } else {
                        Some(IconName::Dash)
                    };
                    let null = if col.nullable { "?" } else { "" };
                    let label = format!("{} ({}){null}", col.name, col.data_type);
                    rows.push(self.render_tree_row(
                        &format!("col:{qualified}.{}", col.name),
                        &label,
                        depth + 2,
                        false,
                        col_icon,
                        cx,
                    ));
                }
            }

            // Constraints
            let con_id = format!("constraints:{qualified}");
            rows.push(self.render_tree_row(
                &con_id,
                &format!("Constraints ({})", table.constraints.len()),
                depth + 1,
                !table.constraints.is_empty(),
                Some(IconName::LockOutlined),
                cx,
            ));
            if self.is_expanded(&con_id) {
                for c in &table.constraints {
                    rows.push(self.render_tree_row(
                        &format!("con:{qualified}.{}", c.name),
                        &format!("{} ({})", c.name, c.kind.label()),
                        depth + 2,
                        false,
                        Some(IconName::LockOutlined),
                        cx,
                    ));
                }
            }

            // Foreign Keys
            let fk_id = format!("fks:{qualified}");
            rows.push(self.render_tree_row(
                &fk_id,
                &format!("Foreign Keys ({})", table.foreign_keys.len()),
                depth + 1,
                !table.foreign_keys.is_empty(),
                Some(IconName::Link),
                cx,
            ));
            if self.is_expanded(&fk_id) {
                for fk in &table.foreign_keys {
                    rows.push(self.render_tree_row(
                        &format!("fk:{qualified}.{}", fk.name),
                        &format!("{} -> {}", fk.name, fk.referenced_table),
                        depth + 2,
                        false,
                        Some(IconName::Link),
                        cx,
                    ));
                }
            }

            // Indexes
            let idx_id = format!("indexes:{qualified}");
            rows.push(self.render_tree_row(
                &idx_id,
                &format!("Indexes ({})", table.indexes.len()),
                depth + 1,
                !table.indexes.is_empty(),
                Some(IconName::ListFilter),
                cx,
            ));
            if self.is_expanded(&idx_id) {
                for idx in &table.indexes {
                    let unique = if idx.is_unique { ", unique" } else { "" };
                    rows.push(self.render_tree_row(
                        &format!("idx:{qualified}.{}", idx.name),
                        &format!("{} ({}{})", idx.name, idx.index_type, unique),
                        depth + 2,
                        false,
                        Some(IconName::ListFilter),
                        cx,
                    ));
                }
            }

            // Triggers
            let trig_id = format!("triggers:{qualified}");
            rows.push(self.render_tree_row(
                &trig_id,
                &format!("Triggers ({})", table.triggers.len()),
                depth + 1,
                !table.triggers.is_empty(),
                Some(IconName::DatabaseZap),
                cx,
            ));
            if self.is_expanded(&trig_id) {
                for t in &table.triggers {
                    rows.push(self.render_trigger_row(
                        &format!("trig:{qualified}.{}", t.name),
                        &format!("{} ({} {})", t.name, t.timing, t.event),
                        &t.name,
                        &table.info.name,
                        schema_name,
                        depth + 2,
                        cx,
                    ));
                }
            }
        }
    }

    fn render_schema_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(tree) = &self.schema_tree else {
            return div().child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().colors().text_disabled)
                    .child("Loading schema..."),
            );
        };
        let Some(profile) = &self.connected_profile else {
            return div();
        };

        let mut rows: Vec<AnyElement> = Vec::new();

        // Connection root
        let conn_id = format!("conn:{}", profile.name);
        rows.push(self.render_tree_row(
            &conn_id,
            &format!("{} (connected)", profile.name),
            0,
            true,
            Some(IconName::DatabaseZap),
            cx,
        ));

        if self.is_expanded(&conn_id) {
            // Database level
            let db_id = format!("db:{}", profile.database);
            rows.push(self.render_tree_row(
                &db_id,
                &profile.database,
                1,
                true,
                Some(IconName::Server),
                cx,
            ));

            if self.is_expanded(&db_id) {
                for schema in &tree.schemas {
                    let schema_id = format!("schema:{}", schema.info.name);
                    rows.push(self.render_tree_row(
                        &schema_id,
                        &schema.info.name,
                        2,
                        true,
                        Some(IconName::Folder),
                        cx,
                    ));

                    if self.is_expanded(&schema_id) {
                        // Tables category
                        let tables_id = format!("tables:{}", schema.info.name);
                        rows.push(self.render_tree_row(
                            &tables_id,
                            &format!("Tables ({})", schema.tables.len()),
                            3,
                            !schema.tables.is_empty(),
                            Some(IconName::ListTree),
                            cx,
                        ));

                        if self.is_expanded(&tables_id) {
                            for table in &schema.tables {
                                self.render_table_entry(table, &schema.info.name, 4, &mut rows, cx);
                            }
                        }

                        // Views
                        let views_id = format!("views:{}", schema.info.name);
                        rows.push(self.render_tree_row(
                            &views_id,
                            &format!("Views ({})", schema.views.len()),
                            3,
                            !schema.views.is_empty(),
                            Some(IconName::Eye),
                            cx,
                        ));
                        if self.is_expanded(&views_id) {
                            for view in &schema.views {
                                self.render_table_entry(view, &schema.info.name, 4, &mut rows, cx);
                            }
                        }

                        // Materialized Views
                        let mv_id = format!("matviews:{}", schema.info.name);
                        rows.push(self.render_tree_row(
                            &mv_id,
                            &format!("Materialized Views ({})", schema.materialized_views.len()),
                            3,
                            !schema.materialized_views.is_empty(),
                            Some(IconName::Eye),
                            cx,
                        ));
                        if self.is_expanded(&mv_id) {
                            for mv in &schema.materialized_views {
                                self.render_table_entry(mv, &schema.info.name, 4, &mut rows, cx);
                            }
                        }

                        // Functions
                        let fn_id = format!("functions:{}", schema.info.name);
                        rows.push(self.render_tree_row(
                            &fn_id,
                            &format!("Functions ({})", schema.functions.len()),
                            3,
                            !schema.functions.is_empty(),
                            Some(IconName::Code),
                            cx,
                        ));
                        if self.is_expanded(&fn_id) {
                            for func in &schema.functions {
                                let label = format!(
                                    "{}({}) -> {}",
                                    func.name, func.arguments, func.return_type
                                );
                                rows.push(self.render_function_row(
                                    &format!("func:{}.{}", schema.info.name, func.name),
                                    &label,
                                    &func.name,
                                    &schema.info.name,
                                    4,
                                    cx,
                                ));
                            }
                        }

                        // Sequences
                        let seq_id = format!("sequences:{}", schema.info.name);
                        rows.push(self.render_tree_row(
                            &seq_id,
                            &format!("Sequences ({})", schema.sequences.len()),
                            3,
                            !schema.sequences.is_empty(),
                            Some(IconName::Hash),
                            cx,
                        ));
                        if self.is_expanded(&seq_id) {
                            for seq in &schema.sequences {
                                rows.push(self.render_tree_row(
                                    &format!("seq:{}.{}", schema.info.name, seq.name),
                                    &seq.name,
                                    4,
                                    false,
                                    Some(IconName::Hash),
                                    cx,
                                ));
                            }
                        }
                    }
                }
            }
        }

        div().flex().flex_col().children(rows)
    }

    /// Render a compact toolbar with quick action icon buttons for common database tasks.
    /// Shown when connected, between the connection info bar and the schema tree.
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .flex_wrap()
            .child(
                IconButton::new("tb-sessions", IconName::Person)
                    .icon_size(IconSize::XSmall)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("Active Sessions"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.execute_in_result_panel(
                            "SELECT pid, usename AS user_name, datname AS database, state, \
                             CASE WHEN state = 'active' THEN (now() - query_start)::text ELSE '' END AS duration, \
                             LEFT(query, 200) AS query \
                             FROM pg_stat_activity WHERE datname IS NOT NULL ORDER BY state DESC, query_start",
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                IconButton::new("tb-locks", IconName::LockOutlined)
                    .icon_size(IconSize::XSmall)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("View Locks"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.execute_in_result_panel(
                            "SELECT blocked_locks.pid AS blocked_pid, \
                             LEFT(blocked_activity.query, 100) AS blocked_query, \
                             blocking_locks.pid AS blocking_pid, \
                             LEFT(blocking_activity.query, 100) AS blocking_query \
                             FROM pg_catalog.pg_locks blocked_locks \
                             JOIN pg_catalog.pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_locks.pid \
                             JOIN pg_catalog.pg_locks blocking_locks ON blocking_locks.locktype = blocked_locks.locktype \
                                 AND blocking_locks.database IS NOT DISTINCT FROM blocked_locks.database \
                                 AND blocking_locks.relation IS NOT DISTINCT FROM blocked_locks.relation \
                                 AND blocking_locks.pid != blocked_locks.pid \
                             JOIN pg_catalog.pg_stat_activity blocking_activity ON blocking_activity.pid = blocking_locks.pid \
                             WHERE NOT blocked_locks.granted",
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                IconButton::new("tb-sizes", IconName::DatabaseZap)
                    .icon_size(IconSize::XSmall)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("Table Sizes"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.execute_in_result_panel(
                            "SELECT schemaname || '.' || tablename AS table_name, \
                             pg_size_pretty(pg_total_relation_size(schemaname || '.' || tablename)) AS total_size, \
                             (SELECT reltuples::bigint FROM pg_class WHERE relname = tablename) AS est_rows \
                             FROM pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema') \
                             ORDER BY pg_total_relation_size(schemaname || '.' || tablename) DESC LIMIT 20",
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                IconButton::new("tb-slow", IconName::Clock)
                    .icon_size(IconSize::XSmall)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("Slow Queries"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.execute_in_result_panel(
                            "SELECT pid, now() - query_start AS duration, usename, LEFT(query, 200) AS query \
                             FROM pg_stat_activity WHERE state != 'idle' AND query NOT ILIKE '%pg_stat_activity%' \
                             ORDER BY duration DESC LIMIT 10",
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                IconButton::new("tb-bloat", IconName::Flame)
                    .icon_size(IconSize::XSmall)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("Table Bloat"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.execute_in_result_panel(
                            "SELECT schemaname || '.' || tablename AS table_name, \
                             n_dead_tup AS dead_rows, n_live_tup AS live_rows, \
                             CASE WHEN n_live_tup > 0 THEN round(100.0 * n_dead_tup / n_live_tup, 1) ELSE 0 END AS bloat_pct \
                             FROM pg_stat_user_tables WHERE n_dead_tup > 100 ORDER BY n_dead_tup DESC LIMIT 10",
                            window,
                            cx,
                        );
                    })),
            )
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .items_center()
            .justify_between()
            .px_2()
            .h(px(30.))
            .border_b_1()
            .border_color(cx.theme().colors().border);

        if let Some(profile) = &self.connected_profile {
            // Connected: show database name + schema summary + disconnect/refresh buttons
            let profile_name = profile.name.clone();

            // Build schema summary string
            let schema_summary = self.schema_tree.as_ref().map(|tree| {
                let mut table_count = 0;
                let mut view_count = 0;
                let mut func_count = 0;
                for schema in &tree.schemas {
                    table_count += schema.tables.len();
                    view_count += schema.views.len();
                    func_count += schema.functions.len();
                }
                format!(
                    "{} tables, {} views, {} functions",
                    table_count, view_count, func_count
                )
            });

            header
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(gpui::green()))
                        .child(
                            Label::new(profile_name)
                                .size(LabelSize::Small)
                                .weight(FontWeight::SEMIBOLD),
                        )
                        .children(schema_summary.map(|summary| {
                            Label::new(summary)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                        })),
                )
                .child(
                    h_flex()
                        .gap_0p5()
                        .child(
                            IconButton::new("refresh-btn", IconName::RefreshTitle)
                                .icon_size(IconSize::XSmall)
                                .icon_color(Color::Muted)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("Refresh Schema"))
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.fetch_schema(cx);
                                })),
                        )
                        .child(
                            IconButton::new("disconnect-btn", IconName::XCircle)
                                .icon_size(IconSize::XSmall)
                                .icon_color(Color::Muted)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("Disconnect"))
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.disconnect(cx);
                                })),
                        )
                        .child(
                            IconButton::new("add-connection-btn", IconName::Plus)
                                .icon_size(IconSize::XSmall)
                                .icon_color(Color::Muted)
                                .style(ButtonStyle::Subtle)
                                .tooltip(Tooltip::text("New Connection"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_form(window, cx);
                                })),
                        ),
                )
        } else {
            // Not connected: show title + add button
            header
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Icon::new(IconName::DatabaseZap)
                                .size(IconSize::XSmall)
                                .color(Color::Muted),
                        )
                        .child(
                            Label::new("DATABASE")
                                .size(LabelSize::XSmall)
                                .weight(FontWeight::SEMIBOLD)
                                .color(Color::Muted),
                        ),
                )
                .child(
                    IconButton::new("add-connection-btn", IconName::Plus)
                        .icon_size(IconSize::XSmall)
                        .icon_color(Color::Muted)
                        .style(ButtonStyle::Subtle)
                        .tooltip(Tooltip::text("New Connection"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_form(window, cx);
                        })),
                )
        }
    }

    fn render_connection_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex();

        if self.saved_connections.is_empty() {
            list = list.child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .py_4()
                    .gap_2()
                    .child(
                        Icon::new(IconName::DatabaseZap)
                            .size(IconSize::Medium)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("No connections")
                            .size(LabelSize::Default)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("Click + to add a PostgreSQL connection")
                            .size(LabelSize::XSmall)
                            .color(Color::Disabled),
                    ),
            );
        }

        for (i, conn) in self.saved_connections.iter().enumerate() {
            let env_color = match conn.environment {
                Environment::Production => rgb(0xf14c4c),
                Environment::Staging => rgb(0xccaa00),
                _ => rgb(0x4ec94e),
            };
            let name = conn.name.clone();
            let host_info = format!("{}:{} / {}", conn.host, conn.port, conn.database);
            let env_label = match conn.environment {
                Environment::Production => "PROD",
                Environment::Staging => "STG",
                Environment::Development => "DEV",
                Environment::Local => "LOCAL",
            };
            let idx = i;
            let connect_idx = i;
            let edit_idx = i;

            let connection_row = ListItem::new(SharedString::from(format!("conn-{i}")))
                .spacing(ListItemSpacing::Dense)
                .inset(true)
                .start_slot(
                    div()
                        .w(px(8.))
                        .h(px(8.))
                        .rounded_full()
                        .bg(env_color)
                        .flex_shrink_0(),
                )
                .end_slot_on_hover(
                    IconButton::new(SharedString::from(format!("del-conn-{i}")), IconName::Trash)
                        .icon_size(IconSize::XSmall)
                        .icon_color(Color::Error)
                        .style(ButtonStyle::Subtle)
                        .tooltip(Tooltip::text("Delete Connection"))
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.delete_connection(idx, cx);
                        })),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.connect_saved(connect_idx, window, cx);
                }))
                .child(
                    v_flex()
                        .child(
                            h_flex()
                                .gap_1()
                                .child(Label::new(name).size(LabelSize::Small))
                                .child({
                                    let badge_bg: Hsla = env_color.into();
                                    let faded_bg = Hsla {
                                        a: 0.15,
                                        ..badge_bg
                                    };
                                    div()
                                        .px(px(4.))
                                        .py(px(1.))
                                        .rounded(px(3.))
                                        .bg(faded_bg)
                                        .child(
                                            Label::new(env_label)
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        )
                                }),
                        )
                        .child(
                            Label::new(host_info)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        ),
                );

            let panel_handle = cx.entity().downgrade();
            list = list.child(
                right_click_menu(SharedString::from(format!("conn-ctx-{i}")))
                    .trigger(move |_, _, _| connection_row)
                    .menu(move |window, cx| {
                        let entity_connect = panel_handle.clone();
                        let entity_edit = panel_handle.clone();
                        let entity_delete = panel_handle.clone();
                        ContextMenu::build(window, cx, move |menu, _window, _cx| {
                            menu.entry("Connect", None, move |window, cx| {
                                if let Some(panel) = entity_connect.upgrade() {
                                    panel.update(cx, |panel, cx| {
                                        panel.connect_saved(connect_idx, window, cx);
                                    });
                                }
                            })
                            .entry("Edit Connection", None, move |window, cx| {
                                if let Some(panel) = entity_edit.upgrade() {
                                    panel.update(cx, |panel, cx| {
                                        if let Some(profile) =
                                            panel.saved_connections.get(edit_idx).cloned()
                                        {
                                            panel.prefill_form_from_profile(&profile, window, cx);
                                            panel.show_form = true;
                                            panel.error_message = None;
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .separator()
                            .entry(
                                "Delete Connection",
                                None,
                                move |_window, cx| {
                                    if let Some(panel) = entity_delete.upgrade() {
                                        panel.update(cx, |panel, cx| {
                                            panel.delete_connection(idx, cx);
                                        });
                                    }
                                },
                            )
                        })
                    }),
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
        v_flex()
            .gap(px(4.))
            .child(
                Label::new(label.to_string())
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
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

        let mut row = h_flex().gap_1();

        for (env, label) in environments {
            let is_selected = self.form_environment == env;
            row = row.child(
                Button::new(SharedString::from(format!("env-{label}")), label)
                    .label_size(LabelSize::XSmall)
                    .style(if is_selected {
                        ButtonStyle::Filled
                    } else {
                        ButtonStyle::Subtle
                    })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.form_environment = env;
                        cx.notify();
                    })),
            );
        }

        v_flex()
            .gap(px(4.))
            .child(
                Label::new("Environment")
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
            .child(row)
    }

    fn render_ssl_mode_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let modes = [
            (SslMode::Disable, "Disable"),
            (SslMode::Prefer, "Prefer"),
            (SslMode::Require, "Require"),
        ];

        let mut row = h_flex().gap_1();

        for (mode, label) in modes {
            let is_selected = self.form_ssl_mode == mode;
            row = row.child(
                Button::new(SharedString::from(format!("ssl-{label}")), label)
                    .label_size(LabelSize::XSmall)
                    .style(if is_selected {
                        ButtonStyle::Filled
                    } else {
                        ButtonStyle::Subtle
                    })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.form_ssl_mode = mode;
                        cx.notify();
                    })),
            );
        }

        v_flex()
            .gap(px(4.))
            .child(
                Label::new("SSL Mode")
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
            .child(row)
    }

    fn render_ssh_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ssh_enabled = self.ssh_enabled;
        let ssh_auth = self.ssh_auth.clone();

        let mut section = v_flex().gap_2().child(
            h_flex()
                .id("ssh-toggle")
                .items_center()
                .gap_2()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _window, cx| {
                    this.ssh_enabled = !this.ssh_enabled;
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(14.))
                        .h(px(14.))
                        .rounded_sm()
                        .border_1()
                        .border_color(cx.theme().colors().border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(ssh_enabled, |s| {
                            s.bg(cx.theme().colors().element_active).child(
                                Icon::new(IconName::Check)
                                    .size(IconSize::XSmall)
                                    .color(Color::Default),
                            )
                        }),
                )
                .child(Label::new("SSH Tunnel").size(LabelSize::Small)),
        );

        if self.ssh_enabled {
            section = section
                .child(self.render_editor_field("SSH Host", &self.ssh_host_editor, cx))
                .child(self.render_editor_field("SSH Port", &self.ssh_port_editor, cx))
                .child(self.render_editor_field("SSH Username", &self.ssh_username_editor, cx))
                .child(self.render_ssh_auth_selector(cx))
                .when(matches!(ssh_auth, SshAuth::KeyFile { .. }), |s| {
                    s.child(self.render_editor_field("Key File Path", &self.ssh_key_editor, cx))
                });
        }

        section
    }

    fn render_ssh_auth_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_agent = matches!(self.ssh_auth, SshAuth::Agent);
        let is_keyfile = matches!(self.ssh_auth, SshAuth::KeyFile { .. });

        v_flex()
            .gap(px(4.))
            .child(
                Label::new("Auth Method")
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("ssh-auth-agent", "Agent")
                            .label_size(LabelSize::XSmall)
                            .style(if is_agent {
                                ButtonStyle::Filled
                            } else {
                                ButtonStyle::Subtle
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.ssh_auth = SshAuth::Agent;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("ssh-auth-keyfile", "Key File")
                            .label_size(LabelSize::XSmall)
                            .style(if is_keyfile {
                                ButtonStyle::Filled
                            } else {
                                ButtonStyle::Subtle
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.ssh_auth = SshAuth::KeyFile {
                                    path: String::new(),
                                };
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let form_title = if self.editing_connection_id.is_some() {
            "Edit Connection"
        } else {
            "New Connection"
        };
        let save_label = if self.editing_connection_id.is_some() {
            "Update & Connect"
        } else {
            "Save & Connect"
        };

        v_flex()
            .p_2()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().surface_background)
            .child(
                Label::new(form_title)
                    .size(LabelSize::Small)
                    .weight(FontWeight::SEMIBOLD),
            )
            .child(self.render_editor_field("Host", &self.host_editor, cx))
            .child(self.render_editor_field("Port", &self.port_editor, cx))
            .child(self.render_editor_field("Database", &self.database_editor, cx))
            .child(self.render_editor_field("Username", &self.username_editor, cx))
            .child(self.render_editor_field("Password", &self.password_editor, cx))
            .child(self.render_environment_selector(cx))
            .child(self.render_ssl_mode_selector(cx))
            .child(self.render_ssh_section(cx))
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.show_form = false;
                                this.editing_connection_id = None;
                                this.error_message = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("test-connection", "Test")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .tooltip(Tooltip::text("Test connection without saving"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.test_connection(window, cx);
                            })),
                    )
                    .child(
                        Button::new("save-connect", save_label)
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
            .child(self.render_header(cx));

        // Show error/success message if present
        if let Some(msg) = &self.error_message {
            let is_success = msg.starts_with("Connection successful") || msg.starts_with("Testing");
            let bg = if is_success {
                cx.theme().status().success_background
            } else {
                cx.theme().status().error_background
            };
            let border = if is_success {
                cx.theme().status().success_border
            } else {
                cx.theme().status().error_border
            };
            let text_color = if is_success {
                cx.theme().status().success
            } else {
                cx.theme().status().error
            };
            panel = panel.child(
                div()
                    .mx_2()
                    .mb_2()
                    .p_2()
                    .rounded_sm()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(text_color)
                    .child(msg.clone()),
            );
        }

        // Connection info bar (shown when connected, before schema tree)
        if let Some(profile) = &self.connected_profile {
            let uptime = self
                .connected_at
                .map(|t| {
                    let secs = t.elapsed().as_secs();
                    if secs < 60 {
                        format!("{}s", secs)
                    } else if secs < 3600 {
                        format!("{}m", secs / 60)
                    } else {
                        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                    }
                })
                .unwrap_or_default();

            let host_line = if uptime.is_empty() {
                format!("{}:{}", profile.host, profile.port)
            } else {
                format!("{}:{} | {}", profile.host, profile.port, uptime)
            };

            panel = panel.child(
                v_flex()
                    .px_2()
                    .py_1()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        Label::new(host_line)
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new(format!("{} / {}", profile.database, profile.username))
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            );
        }

        if self.show_form {
            panel = panel.child(self.render_form(cx));
        } else if self.schema_tree.is_some() {
            // Connected with schema loaded — show toolbar + tree
            panel = panel.child(self.render_toolbar(cx));
            panel = panel.child(
                div()
                    .id("schema-tree-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .py_1()
                    .child(self.render_schema_tree(cx)),
            );
        } else if self.session.is_some() {
            // Connected but schema still loading — show toolbar
            panel = panel.child(self.render_toolbar(cx));
            panel = panel.child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child("Loading schema..."),
            );
        } else {
            // Not connected — show connection list
            panel = panel.child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .py_1()
                    .child(self.render_connection_list(cx)),
            );
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
