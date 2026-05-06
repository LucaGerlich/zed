use std::collections::HashSet;
use std::sync::Arc;

use editor::Editor;
use gpui::*;
use tokio::runtime::Runtime;
use ui::prelude::*;
use ui::{Button, ButtonStyle, IconName};
use workspace::Workspace;
use workspace::dock::{DockPosition, Panel, PanelEvent};

use pgblade_core::connection::{ConnectionProfile, Environment, SshAuth, SshConfig};
use pgblade_core::driver::{DatabaseDriver, DatabaseSession};
use pgblade_core::schema::{SchemaTree, TableEntry, TableKind};
use pgblade_core::security::CredentialStore;
use pgblade_core::ssh::SshTunnel;
use pgblade_core::storage::StorageManager;
use pgblade_postgres::PostgresDriver;
use pgblade_security::KeychainStore;

use crate::ResultPanel;

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
}

impl ConnectionPanel {
    pub fn new(
        workspace: WeakEntity<Workspace>,
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
            ssh,
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

        // Connect to the database
        self.connect(profile, password, cx);
    }

    fn connect_saved(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(profile) = self.saved_connections.get(index).cloned() else {
            return;
        };
        let password = self
            .credential_store
            .retrieve(&profile.keychain_service_key())
            .ok()
            .flatten()
            .unwrap_or_default();
        self.connect(profile, password, cx);
    }

    fn connect(
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

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { driver.connect(&connect_profile, &password).await })
                .await;

            match result {
                Ok(Ok(session)) => {
                    let session: Arc<dyn DatabaseSession> = Arc::from(session);
                    this.update(cx, |panel, cx| {
                        panel.session = Some(session);
                        panel.connected_profile = Some(profile);
                        tracing::info!("database connection established");
                        cx.emit(PanelEvent::Activate);
                        cx.notify();
                        panel.fetch_schema(cx);
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

    fn fetch_schema(&mut self, cx: &mut Context<Self>) {
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

    /// Returns the active database session, if connected.
    pub fn session(&self) -> Option<Arc<dyn DatabaseSession>> {
        self.session.clone()
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

    /// Disconnect from the current database session and reset state.
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        self.connected_profile = None;
        self.ssh_tunnel = None; // Kills the SSH process via Drop
        self.schema_tree = None;
        self.expanded_nodes.clear();
        self.error_message = None;
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

    fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded_nodes.contains(node_id)
    }

    /// Preview table data by executing SELECT * FROM ... LIMIT 100
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
        let sql = format!("SELECT * FROM \"{schema}\".\"{table}\" LIMIT 100");

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
                if let Some(active_item) = workspace.active_item(cx) {
                    if let Some(editor) = active_item.act_as::<Editor>(cx) {
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

    /// Insert SQL text into the active editor at the end.
    fn insert_sql_into_editor(&self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace.upgrade() {
            let sql = sql.to_string();
            workspace.update(cx, |workspace, cx| {
                if let Some(active_item) = workspace.active_item(cx) {
                    if let Some(editor) = active_item.act_as::<Editor>(cx) {
                        editor.update(cx, |editor, cx| {
                            let text = editor.text(cx);
                            let prefix = if text.is_empty() { "" } else { "\n\n" };
                            editor.move_to_end(&editor::actions::MoveToEnd, window, cx);
                            editor.insert(&format!("{prefix}{sql}"), window, cx);
                        });
                    }
                }
            });
        }
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

    /// Generate CREATE TABLE DDL from introspected schema information.
    fn generate_ddl(table: &TableEntry) -> String {
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = depth as f32 * 12.0;
        let is_expanded = self.expanded_nodes.contains(node_id);
        let chevron = if has_children {
            if is_expanded { "v " } else { "> " }
        } else {
            "  "
        };

        let id = SharedString::from(format!("tree-{node_id}"));
        let node_id_owned = node_id.to_string();

        div()
            .id(id)
            .h(px(22.))
            .flex()
            .flex_row()
            .items_center()
            .pl(px(indent + 4.0))
            .pr_1()
            .text_xs()
            .text_color(cx.theme().colors().text)
            .hover(|s| s.bg(cx.theme().colors().element_hover))
            .cursor_pointer()
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
                div()
                    .text_color(cx.theme().colors().text_muted)
                    .child(chevron.to_string()),
            )
            .child(label.to_string())
            .into_any_element()
    }

    /// Render a table row that expands on click AND triggers data preview.
    /// Also includes DDL, SQL, INS, and UPD buttons visible on hover.
    fn render_table_row(
        &self,
        node_id: &str,
        table_name: &str,
        schema_name: &str,
        columns: &[String],
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = depth as f32 * 12.0;
        let is_expanded = self.expanded_nodes.contains(node_id);
        let chevron = if is_expanded { "v " } else { "> " };

        let id = SharedString::from(format!("tree-{node_id}"));
        let ddl_id = SharedString::from(format!("ddl-{node_id}"));
        let sql_id = SharedString::from(format!("sql-{node_id}"));
        let ins_id = SharedString::from(format!("ins-{node_id}"));
        let upd_id = SharedString::from(format!("upd-{node_id}"));
        let node_id_owned = node_id.to_string();
        let schema_owned = schema_name.to_string();
        let table_owned = table_name.to_string();
        let schema_for_ddl = schema_name.to_string();
        let table_for_ddl = table_name.to_string();
        let schema_for_sql = schema_name.to_string();
        let table_for_sql = table_name.to_string();
        let schema_for_ins = schema_name.to_string();
        let table_for_ins = table_name.to_string();
        let cols_for_ins = columns.to_vec();
        let schema_for_upd = schema_name.to_string();
        let table_for_upd = table_name.to_string();
        let cols_for_upd = columns.to_vec();

        div()
            .id(id)
            .h(px(22.))
            .flex()
            .flex_row()
            .items_center()
            .pl(px(indent + 4.0))
            .pr_1()
            .text_xs()
            .text_color(cx.theme().colors().text)
            .hover(|s| s.bg(cx.theme().colors().element_hover))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                // Toggle expand/collapse
                if this.expanded_nodes.contains(&node_id_owned) {
                    this.expanded_nodes.remove(&node_id_owned);
                } else {
                    this.expanded_nodes.insert(node_id_owned.clone());
                }
                cx.notify();
                // Preview table data
                this.preview_table(&schema_owned, &table_owned, window, cx);
            }))
            .child(
                div()
                    .text_color(cx.theme().colors().text_muted)
                    .child(chevron.to_string()),
            )
            .child(div().flex_1().child(table_name.to_string()))
            .child(
                div()
                    .id(sql_id)
                    .text_xs()
                    .mr_1()
                    .text_color(cx.theme().colors().text_disabled)
                    .hover(|s| s.text_color(cx.theme().colors().text))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_select(&schema_for_sql, &table_for_sql, window, cx);
                    }))
                    .child("SEL"),
            )
            .child(
                div()
                    .id(ins_id)
                    .text_xs()
                    .mr_1()
                    .text_color(cx.theme().colors().text_disabled)
                    .hover(|s| s.text_color(cx.theme().colors().text))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_insert(
                            &schema_for_ins,
                            &table_for_ins,
                            &cols_for_ins,
                            window,
                            cx,
                        );
                    }))
                    .child("INS"),
            )
            .child(
                div()
                    .id(upd_id)
                    .text_xs()
                    .mr_1()
                    .text_color(cx.theme().colors().text_disabled)
                    .hover(|s| s.text_color(cx.theme().colors().text))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.generate_update(
                            &schema_for_upd,
                            &table_for_upd,
                            &cols_for_upd,
                            window,
                            cx,
                        );
                    }))
                    .child("UPD"),
            )
            .child(
                div()
                    .id(ddl_id)
                    .text_xs()
                    .text_color(cx.theme().colors().text_disabled)
                    .hover(|s| s.text_color(cx.theme().colors().text))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_ddl(&schema_for_ddl, &table_for_ddl, window, cx);
                    }))
                    .child("DDL"),
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
        rows.push(self.render_table_row(
            &table_id,
            &table.info.name,
            schema_name,
            &col_names,
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
                cx,
            ));
            if self.is_expanded(&cols_id) {
                for col in &table.columns {
                    let pk = if col.is_primary_key { "PK " } else { "" };
                    let null = if col.nullable { "?" } else { "" };
                    let label = format!("{pk}{} ({}){null}", col.name, col.data_type);
                    rows.push(self.render_tree_row(
                        &format!("col:{qualified}.{}", col.name),
                        &label,
                        depth + 2,
                        false,
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
                cx,
            ));
            if self.is_expanded(&con_id) {
                for c in &table.constraints {
                    rows.push(self.render_tree_row(
                        &format!("con:{qualified}.{}", c.name),
                        &format!("{} ({})", c.name, c.kind.label()),
                        depth + 2,
                        false,
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
                cx,
            ));
            if self.is_expanded(&fk_id) {
                for fk in &table.foreign_keys {
                    rows.push(self.render_tree_row(
                        &format!("fk:{qualified}.{}", fk.name),
                        &format!("{} -> {}", fk.name, fk.referenced_table),
                        depth + 2,
                        false,
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
                cx,
            ));
            if self.is_expanded(&trig_id) {
                for t in &table.triggers {
                    rows.push(self.render_tree_row(
                        &format!("trig:{qualified}.{}", t.name),
                        &format!("{} ({} {})", t.name, t.timing, t.event),
                        depth + 2,
                        false,
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
            cx,
        ));

        if self.is_expanded(&conn_id) {
            // Database level
            let db_id = format!("db:{}", profile.database);
            rows.push(self.render_tree_row(&db_id, &profile.database, 1, true, cx));

            if self.is_expanded(&db_id) {
                for schema in &tree.schemas {
                    let schema_id = format!("schema:{}", schema.info.name);
                    rows.push(self.render_tree_row(&schema_id, &schema.info.name, 2, true, cx));

                    if self.is_expanded(&schema_id) {
                        // Tables category
                        let tables_id = format!("tables:{}", schema.info.name);
                        rows.push(self.render_tree_row(
                            &tables_id,
                            &format!("Tables ({})", schema.tables.len()),
                            3,
                            !schema.tables.is_empty(),
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
                            cx,
                        ));
                        if self.is_expanded(&fn_id) {
                            for func in &schema.functions {
                                let label = format!(
                                    "{}({}) -> {}",
                                    func.name, func.arguments, func.return_type
                                );
                                rows.push(self.render_tree_row(
                                    &format!("func:{}.{}", schema.info.name, func.name),
                                    &label,
                                    4,
                                    false,
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
                            cx,
                        ));
                        if self.is_expanded(&seq_id) {
                            for seq in &schema.sequences {
                                rows.push(self.render_tree_row(
                                    &format!("seq:{}.{}", schema.info.name, seq.name),
                                    &seq.name,
                                    4,
                                    false,
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

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_2()
            .py_1();

        if let Some(profile) = &self.connected_profile {
            // Connected: show database name + disconnect/refresh buttons
            let profile_name = profile.name.clone();
            header
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(gpui::green()))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().colors().text)
                                .child(profile_name),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_1()
                        .child(
                            div()
                                .id("refresh-btn")
                                .cursor_pointer()
                                .text_xs()
                                .text_color(cx.theme().colors().text_muted)
                                .hover(|s| s.text_color(cx.theme().colors().text))
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.fetch_schema(cx);
                                }))
                                .child("Refresh"),
                        )
                        .child(
                            div()
                                .id("disconnect-btn")
                                .cursor_pointer()
                                .text_xs()
                                .text_color(cx.theme().colors().text_muted)
                                .hover(|s| s.text_color(gpui::red()))
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.disconnect(cx);
                                }))
                                .child("Disconnect"),
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
                        ),
                )
        } else {
            // Not connected: show title + add button
            header
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

            let connect_idx = i;
            list = list.child(
                div()
                    .id(SharedString::from(format!("conn-{i}")))
                    .h(px(26.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().colors().element_active))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.connect_saved(connect_idx, cx);
                    }))
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

    fn render_ssh_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ssh_enabled = self.ssh_enabled;
        let ssh_auth = self.ssh_auth.clone();

        let mut section = div().flex().flex_col().gap_2().child(
            div()
                .id("ssh-toggle")
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _window, cx| {
                    this.ssh_enabled = !this.ssh_enabled;
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(12.))
                        .h(px(12.))
                        .rounded_sm()
                        .border_1()
                        .border_color(cx.theme().colors().border)
                        .when(ssh_enabled, |s| s.bg(cx.theme().colors().element_active)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().colors().text)
                        .child("SSH Tunnel"),
                ),
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

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child("Auth Method"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        div()
                            .id("ssh-auth-agent")
                            .px_2()
                            .py(px(2.))
                            .text_xs()
                            .rounded_sm()
                            .cursor_pointer()
                            .when(is_agent, |s| {
                                s.bg(cx.theme().colors().element_active)
                                    .text_color(cx.theme().colors().text)
                            })
                            .when(!is_agent, |s| {
                                s.text_color(cx.theme().colors().text_muted)
                                    .hover(|s| s.bg(cx.theme().colors().element_active))
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.ssh_auth = SshAuth::Agent;
                                cx.notify();
                            }))
                            .child("Agent"),
                    )
                    .child(
                        div()
                            .id("ssh-auth-keyfile")
                            .px_2()
                            .py(px(2.))
                            .text_xs()
                            .rounded_sm()
                            .cursor_pointer()
                            .when(is_keyfile, |s| {
                                s.bg(cx.theme().colors().element_active)
                                    .text_color(cx.theme().colors().text)
                            })
                            .when(!is_keyfile, |s| {
                                s.text_color(cx.theme().colors().text_muted)
                                    .hover(|s| s.bg(cx.theme().colors().element_active))
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.ssh_auth = SshAuth::KeyFile {
                                    path: String::new(),
                                };
                                cx.notify();
                            }))
                            .child("Key File"),
                    ),
            )
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
            .child(self.render_ssh_section(cx))
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
            .child(self.render_header(cx));

        // Show error message if present
        if let Some(error) = &self.error_message {
            panel = panel.child(
                div()
                    .mx_2()
                    .mb_2()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().status().error_background)
                    .border_1()
                    .border_color(cx.theme().status().error_border)
                    .text_xs()
                    .text_color(cx.theme().status().error)
                    .child(error.clone()),
            );
        }

        if self.show_form {
            panel = panel.child(self.render_form(cx));
        } else if self.schema_tree.is_some() {
            // Connected with schema loaded — show tree
            panel = panel.child(
                div()
                    .id("schema-tree-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .py_1()
                    .child(self.render_schema_tree(cx)),
            );
        } else if self.session.is_some() {
            // Connected but schema still loading
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
