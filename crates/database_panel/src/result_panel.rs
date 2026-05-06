use std::sync::Arc;

use gpui::*;
use tokio::runtime::Runtime;
use ui::prelude::*;
use ui::{
    Button, ButtonStyle, IconButton, IconName, IconSize, Label, LabelCommon, LabelSize, Tooltip,
};
use workspace::Workspace;
use workspace::dock::{DockPosition, Panel, PanelEvent};

use pgblade_core::driver::DatabaseSession;
use pgblade_core::result::{CellValue, ColumnMeta};

actions!(database_result_panel, [ToggleFocus]);

pub fn register(workspace: &mut Workspace) {
    workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
        workspace.toggle_panel_focus::<ResultPanel>(window, cx);
    });
}

#[derive(Clone)]
enum ResultState {
    Loading,
    Success {
        columns: Vec<ColumnMeta>,
        rows: Vec<Vec<CellValue>>,
        duration_ms: u128,
    },
    Ddl(String),
    Explain(String),
    Error(String),
}

#[derive(Clone)]
struct ResultTab {
    label: String,
    state: ResultState,
    sort_column: Option<usize>,
    sort_ascending: bool,
    /// The source table (schema, table_name) for row editing.
    source_table: Option<(String, String)>,
    /// The SQL that produced this tab's results (for refresh).
    sql: Option<String>,
    /// The session used to execute the query (for refresh).
    session: Option<Arc<dyn DatabaseSession>>,
    /// The runtime used to execute the query (for refresh).
    runtime: Option<Arc<Runtime>>,
    /// Current row limit for pagination (increases with "Load More").
    row_limit: usize,
    /// Whether the last query returned exactly the limit (more rows likely available).
    has_more: bool,
}

pub struct ResultPanel {
    focus_handle: FocusHandle,
    active: bool,
    tabs: Vec<ResultTab>,
    active_tab: usize,
    /// Currently selected row index (for inline editing).
    selected_row: Option<usize>,
}

impl ResultPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            active: false,
            tabs: Vec::new(),
            active_tab: 0,
            selected_row: None,
        }
    }

    /// Execute a SQL query using the given session and display results in a new tab.
    pub fn execute_query(
        &mut self,
        sql: String,
        session: Arc<dyn DatabaseSession>,
        runtime: Arc<Runtime>,
        cx: &mut Context<Self>,
    ) {
        self.execute_query_with_source(sql, session, runtime, None, cx);
    }

    /// Execute a SQL query with an optional source table for inline editing support.
    pub fn execute_query_with_source(
        &mut self,
        sql: String,
        session: Arc<dyn DatabaseSession>,
        runtime: Arc<Runtime>,
        source_table: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let label = Self::smart_label(&sql);
        self.selected_row = None;
        let row_limit = 500usize;
        self.tabs.push(ResultTab {
            label,
            state: ResultState::Loading,
            sort_column: None,
            sort_ascending: true,
            source_table,
            sql: Some(sql.clone()),
            session: Some(session.clone()),
            runtime: Some(runtime.clone()),
            row_limit,
            has_more: false,
        });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();

        let tab_idx = self.active_tab;
        let started = std::time::Instant::now();

        // Add LIMIT to the query if not already present
        let sql_with_limit = if sql.to_uppercase().contains(" LIMIT ") {
            sql.clone()
        } else {
            format!(
                "{} LIMIT {}",
                sql.trim().trim_end_matches(';'),
                row_limit + 1
            )
        };

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql_with_limit).await })
                .await;

            let elapsed = started.elapsed().as_millis();

            this.update(cx, |panel, cx| {
                if let Some(tab) = panel.tabs.get_mut(tab_idx) {
                    match result {
                        Ok(Ok(mut result_set)) => {
                            let has_more = result_set.rows.len() > row_limit;
                            if has_more {
                                result_set.rows.truncate(row_limit);
                            }
                            tab.state = ResultState::Success {
                                columns: result_set.columns,
                                rows: result_set.rows,
                                duration_ms: elapsed,
                            };
                            tab.has_more = has_more;
                            tab.row_limit = row_limit;
                        }
                        Ok(Err(e)) => {
                            tab.state = ResultState::Error(e.to_string());
                        }
                        Err(e) => {
                            tab.state = ResultState::Error(format!("runtime error: {e}"));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Display pre-built result data directly without executing a query.
    pub fn show_results(
        &mut self,
        columns: Vec<ColumnMeta>,
        rows: Vec<Vec<CellValue>>,
        cx: &mut Context<Self>,
    ) {
        self.selected_row = None;
        self.tabs.push(ResultTab {
            label: "Results".to_string(),
            state: ResultState::Success {
                columns,
                rows,
                duration_ms: 0,
            },
            sort_column: None,
            sort_ascending: true,
            source_table: None,
            sql: None,
            session: None,
            runtime: None,
            row_limit: 500,
            has_more: false,
        });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    /// Display DDL text in the result panel.
    pub fn show_ddl(&mut self, ddl: String, cx: &mut Context<Self>) {
        self.selected_row = None;
        self.tabs.push(ResultTab {
            label: "DDL".to_string(),
            state: ResultState::Ddl(ddl),
            sort_column: None,
            sort_ascending: true,
            source_table: None,
            sql: None,
            session: None,
            runtime: None,
            row_limit: 500,
            has_more: false,
        });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    /// Execute an EXPLAIN ANALYZE query and display the formatted plan.
    pub fn execute_explain(
        &mut self,
        sql: String,
        session: Arc<dyn DatabaseSession>,
        runtime: Arc<Runtime>,
        cx: &mut Context<Self>,
    ) {
        self.selected_row = None;
        self.tabs.push(ResultTab {
            label: "Explain".to_string(),
            state: ResultState::Loading,
            sort_column: None,
            sort_ascending: true,
            source_table: None,
            sql: None,
            session: None,
            runtime: None,
            row_limit: 500,
            has_more: false,
        });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();

        let tab_idx = self.active_tab;

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql).await })
                .await;

            this.update(cx, |panel, cx| {
                if let Some(tab) = panel.tabs.get_mut(tab_idx) {
                    match result {
                        Ok(Ok(result_set)) => {
                            // The JSON plan is in the first column of the first row
                            let plan_text = result_set
                                .rows
                                .first()
                                .and_then(|row| row.first())
                                .map(|cell| cell.display())
                                .unwrap_or_else(|| "No plan available".to_string());

                            let formatted = format_explain_plan(&plan_text);
                            tab.state = ResultState::Explain(formatted);
                        }
                        Ok(Err(e)) => {
                            tab.state = ResultState::Error(e.to_string());
                        }
                        Err(e) => {
                            tab.state = ResultState::Error(format!("runtime error: {e}"));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Generate a concise, meaningful tab label from a SQL statement.
    fn smart_label(sql: &str) -> String {
        let trimmed = sql.trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("SELECT") {
            // Try to extract table name after FROM
            if let Some(from_pos) = upper.find(" FROM ") {
                let after_from = &trimmed[from_pos + 6..];
                let table: String = after_from
                    .split_whitespace()
                    .next()
                    .unwrap_or("?")
                    .trim_matches('"')
                    .to_string();
                return format!("SELECT {table}");
            }
            return "SELECT".to_string();
        }
        if upper.starts_with("INSERT") {
            return "INSERT".to_string();
        }
        if upper.starts_with("UPDATE") {
            return "UPDATE".to_string();
        }
        if upper.starts_with("DELETE") {
            return "DELETE".to_string();
        }
        if upper.starts_with("EXPLAIN") {
            return "EXPLAIN".to_string();
        }
        if upper.starts_with("CREATE") {
            return "CREATE".to_string();
        }

        trimmed.chars().take(25).collect()
    }

    /// Get the active tab's state.
    fn active_state(&self) -> &ResultState {
        &self.tabs[self.active_tab].state
    }

    /// Close a tab by index. Allows closing the last tab (returns to empty state).
    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.tabs.remove(index);
        self.selected_row = None;
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }
        cx.notify();
    }

    /// Re-execute the query that produced the current tab's results.
    fn refresh_current_tab(&mut self, cx: &mut Context<Self>) {
        let tab = &self.tabs[self.active_tab];
        let Some(sql) = tab.sql.clone() else {
            return;
        };
        let Some(session) = tab.session.clone() else {
            return;
        };
        let Some(runtime) = tab.runtime.clone() else {
            return;
        };
        let row_limit = tab.row_limit;

        self.tabs[self.active_tab].state = ResultState::Loading;
        self.tabs[self.active_tab].has_more = false;
        cx.notify();

        let tab_idx = self.active_tab;
        let started = std::time::Instant::now();

        // Add LIMIT to the query if not already present
        let sql_with_limit = if sql.to_uppercase().contains(" LIMIT ") {
            sql.clone()
        } else {
            format!(
                "{} LIMIT {}",
                sql.trim().trim_end_matches(';'),
                row_limit + 1
            )
        };

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql_with_limit).await })
                .await;

            let elapsed = started.elapsed().as_millis();

            this.update(cx, |panel, cx| {
                if let Some(tab) = panel.tabs.get_mut(tab_idx) {
                    match result {
                        Ok(Ok(mut result_set)) => {
                            let has_more = result_set.rows.len() > row_limit;
                            if has_more {
                                result_set.rows.truncate(row_limit);
                            }
                            tab.state = ResultState::Success {
                                columns: result_set.columns,
                                rows: result_set.rows,
                                duration_ms: elapsed,
                            };
                            tab.has_more = has_more;
                            tab.sort_column = None;
                            tab.sort_ascending = true;
                        }
                        Ok(Err(e)) => {
                            tab.state = ResultState::Error(e.to_string());
                        }
                        Err(e) => {
                            tab.state = ResultState::Error(format!("runtime error: {e}"));
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Load more rows by re-executing with a higher limit.
    fn load_more(&mut self, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active_tab];
        let Some(sql) = tab.sql.clone() else {
            return;
        };
        let Some(session) = tab.session.clone() else {
            return;
        };
        let Some(runtime) = tab.runtime.clone() else {
            return;
        };

        // Increase the limit
        let new_limit = tab.row_limit + 500;
        tab.row_limit = new_limit;
        tab.state = ResultState::Loading;
        tab.has_more = false;
        cx.notify();

        let tab_idx = self.active_tab;
        let started = std::time::Instant::now();

        // Re-execute with higher limit
        let sql_with_limit = if sql.to_uppercase().contains(" LIMIT ") {
            sql.clone()
        } else {
            format!(
                "{} LIMIT {}",
                sql.trim().trim_end_matches(';'),
                new_limit + 1
            )
        };

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql_with_limit).await })
                .await;
            let elapsed = started.elapsed().as_millis();

            this.update(cx, |panel, cx| {
                if let Some(tab) = panel.tabs.get_mut(tab_idx) {
                    match result {
                        Ok(Ok(mut result_set)) => {
                            let has_more = result_set.rows.len() > new_limit;
                            if has_more {
                                result_set.rows.truncate(new_limit);
                            }
                            tab.state = ResultState::Success {
                                columns: result_set.columns,
                                rows: result_set.rows,
                                duration_ms: elapsed,
                            };
                            tab.has_more = has_more;
                            tab.sort_column = None;
                            tab.sort_ascending = true;
                        }
                        Ok(Err(e)) => {
                            tab.state = ResultState::Error(e.to_string());
                        }
                        Err(e) => {
                            tab.state = ResultState::Error(format!("runtime error: {e}"));
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Load all rows by re-executing the query without any automatic LIMIT.
    fn load_all(&mut self, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active_tab];
        let Some(sql) = tab.sql.clone() else {
            return;
        };
        let Some(session) = tab.session.clone() else {
            return;
        };
        let Some(runtime) = tab.runtime.clone() else {
            return;
        };

        tab.state = ResultState::Loading;
        tab.has_more = false;
        cx.notify();

        let tab_idx = self.active_tab;
        let started = std::time::Instant::now();

        // Execute original SQL without adding LIMIT
        let sql_no_limit = sql.clone();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql_no_limit).await })
                .await;
            let elapsed = started.elapsed().as_millis();

            this.update(cx, |panel, cx| {
                if let Some(tab) = panel.tabs.get_mut(tab_idx) {
                    match result {
                        Ok(Ok(result_set)) => {
                            tab.state = ResultState::Success {
                                columns: result_set.columns,
                                rows: result_set.rows,
                                duration_ms: elapsed,
                            };
                            tab.has_more = false;
                            tab.row_limit = usize::MAX;
                            tab.sort_column = None;
                            tab.sort_ascending = true;
                        }
                        Ok(Err(e)) => {
                            tab.state = ResultState::Error(e.to_string());
                        }
                        Err(e) => {
                            tab.state = ResultState::Error(format!("runtime error: {e}"));
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Clear all result tabs and reset to empty state.
    fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.tabs.clear();
        self.selected_row = None;
        self.active_tab = 0;
        cx.notify();
    }

    /// Select a row by index for inline editing.
    fn select_row(&mut self, row_idx: usize, cx: &mut Context<Self>) {
        if self.selected_row == Some(row_idx) {
            self.selected_row = None;
        } else {
            self.selected_row = Some(row_idx);
        }
        cx.notify();
    }

    /// Generate an UPDATE statement for the selected row and open it in a new DDL tab.
    fn edit_selected_row(&mut self, cx: &mut Context<Self>) {
        let Some(row_idx) = self.selected_row else {
            return;
        };
        let tab = &self.tabs[self.active_tab];
        let ResultState::Success { columns, rows, .. } = &tab.state else {
            return;
        };
        let sorted = self.sorted_rows(rows);
        let Some(row) = sorted.get(row_idx) else {
            return;
        };

        let table_name = tab
            .source_table
            .as_ref()
            .map(|(s, t)| format!("\"{s}\".\"{t}\""))
            .unwrap_or_else(|| "your_table".to_string());

        let sets: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let val = Self::cell_to_sql_literal(&row[i]);
                format!("    \"{}\" = {}", c.name, val)
            })
            .collect();

        let where_parts: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| match &row[i] {
                CellValue::Null => format!("\"{}\" IS NULL", c.name),
                v => format!("\"{}\" = {}", c.name, Self::cell_to_sql_literal(v)),
            })
            .collect();

        let sql = format!(
            "UPDATE {table_name}\nSET\n{}\nWHERE\n    {};",
            sets.join(",\n"),
            where_parts.join("\n    AND ")
        );

        cx.write_to_clipboard(ClipboardItem::new_string(sql.clone()));

        let source_table = tab.source_table.clone();
        let session = tab.session.clone();
        let runtime = tab.runtime.clone();
        self.tabs.push(ResultTab {
            label: format!("Edit Row {}", row_idx + 1),
            state: ResultState::Ddl(format!(
                "-- Edit the values below and execute with Cmd+Enter\n\
                 -- Original row #{}\n\
                 -- (Also copied to clipboard)\n\n{}",
                row_idx + 1,
                sql
            )),
            sort_column: None,
            sort_ascending: true,
            source_table,
            sql: None,
            session,
            runtime,
            row_limit: 500,
            has_more: false,
        });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    /// Generate a DELETE statement for the selected row and open it in a new DDL tab.
    fn delete_selected_row(&mut self, cx: &mut Context<Self>) {
        let Some(row_idx) = self.selected_row else {
            return;
        };
        let tab = &self.tabs[self.active_tab];
        let ResultState::Success { columns, rows, .. } = &tab.state else {
            return;
        };
        let sorted = self.sorted_rows(rows);
        let Some(row) = sorted.get(row_idx) else {
            return;
        };

        let table_name = tab
            .source_table
            .as_ref()
            .map(|(s, t)| format!("\"{s}\".\"{t}\""))
            .unwrap_or_else(|| "your_table".to_string());

        let where_parts: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| match &row[i] {
                CellValue::Null => format!("\"{}\" IS NULL", c.name),
                v => format!("\"{}\" = {}", c.name, Self::cell_to_sql_literal(v)),
            })
            .collect();

        let sql = format!(
            "DELETE FROM {table_name}\nWHERE\n    {};",
            where_parts.join("\n    AND ")
        );

        cx.write_to_clipboard(ClipboardItem::new_string(sql.clone()));

        let source_table = tab.source_table.clone();
        let session = tab.session.clone();
        let runtime = tab.runtime.clone();
        self.tabs.push(ResultTab {
            label: format!("Delete Row {}", row_idx + 1),
            state: ResultState::Ddl(format!(
                "-- Review the DELETE below and execute with Cmd+Enter\n\
                 -- Original row #{}\n\
                 -- (Also copied to clipboard)\n\n{}",
                row_idx + 1,
                sql
            )),
            sort_column: None,
            sort_ascending: true,
            source_table,
            sql: None,
            session,
            runtime,
            row_limit: 500,
            has_more: false,
        });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    /// Export current result set as CSV to clipboard.
    fn export_csv(&self, cx: &mut Context<Self>) {
        let ResultState::Success { columns, rows, .. } = self.active_state() else {
            return;
        };

        let mut csv = String::new();

        // Header row
        let header: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
        csv.push_str(&header.join(","));
        csv.push('\n');

        // Data rows
        for row in rows {
            let line: Vec<String> = row
                .iter()
                .map(|cell| {
                    let val = cell.display();
                    if val.contains(',') || val.contains('"') || val.contains('\n') {
                        format!("\"{}\"", val.replace('"', "\"\""))
                    } else {
                        val
                    }
                })
                .collect();
            csv.push_str(&line.join(","));
            csv.push('\n');
        }

        cx.write_to_clipboard(ClipboardItem::new_string(csv));
    }

    /// Export current result set as JSON to clipboard.
    fn export_json(&self, cx: &mut Context<Self>) {
        let ResultState::Success { columns, rows, .. } = self.active_state() else {
            return;
        };

        let json_rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, cell) in row.iter().enumerate() {
                    let key = columns.get(i).map(|c| c.name.as_str()).unwrap_or("?");
                    let val = match cell {
                        CellValue::Null => serde_json::Value::Null,
                        CellValue::Boolean(b) => serde_json::Value::Bool(*b),
                        CellValue::Integer(n) => serde_json::json!(*n),
                        CellValue::Float(f) => serde_json::json!(*f),
                        _ => serde_json::Value::String(cell.display()),
                    };
                    obj.insert(key.to_string(), val);
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        let json = serde_json::to_string_pretty(&json_rows).unwrap_or_default();
        cx.write_to_clipboard(ClipboardItem::new_string(json));
    }

    /// Export current result set as INSERT statements to clipboard.
    fn export_insert(&self, cx: &mut Context<Self>) {
        let ResultState::Success { columns, rows, .. } = self.active_state() else {
            return;
        };

        let col_names = columns
            .iter()
            .map(|c| format!("\"{}\"", c.name))
            .collect::<Vec<_>>()
            .join(", ");

        let mut sql = String::new();
        for row in rows {
            let values: Vec<String> = row
                .iter()
                .map(|cell| match cell {
                    CellValue::Null => "NULL".to_string(),
                    CellValue::Boolean(b) => b.to_string(),
                    CellValue::Integer(n) => n.to_string(),
                    CellValue::Float(f) => f.to_string(),
                    CellValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                    CellValue::Json(s) => format!("'{}'", s.replace('\'', "''")),
                    CellValue::Bytes(b) => {
                        let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
                        format!("'\\x{hex}'")
                    }
                    CellValue::Timestamp(s)
                    | CellValue::Date(s)
                    | CellValue::Time(s)
                    | CellValue::Uuid(s) => format!("'{}'", s.replace('\'', "''")),
                    CellValue::Array(items) => {
                        let inner: Vec<String> = items.iter().map(|v| v.display()).collect();
                        format!("'{{{}}}'", inner.join(","))
                    }
                })
                .collect();
            sql.push_str(&format!(
                "INSERT INTO your_table ({col_names}) VALUES ({});\n",
                values.join(", ")
            ));
        }

        cx.write_to_clipboard(ClipboardItem::new_string(sql));
    }

    /// Format a CellValue as a SQL literal for use in generated statements.
    fn cell_to_sql_literal(cell: &CellValue) -> String {
        match cell {
            CellValue::Null => "NULL".to_string(),
            CellValue::Boolean(b) => b.to_string(),
            CellValue::Integer(n) => n.to_string(),
            CellValue::Float(f) => f.to_string(),
            v => format!("'{}'", v.display().replace('\'', "''")),
        }
    }

    /// Export current result set as UPDATE statements to clipboard.
    fn export_update(&self, cx: &mut Context<Self>) {
        let ResultState::Success { columns, rows, .. } = self.active_state() else {
            return;
        };

        let mut sql =
            String::from("-- No source table info. Replace 'your_table' and add WHERE clause.\n\n");
        for row in rows {
            let sets: Vec<String> = columns
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let val = Self::cell_to_sql_literal(&row[i]);
                    format!("    \"{}\" = {}", c.name, val)
                })
                .collect();
            sql.push_str(&format!(
                "UPDATE your_table SET\n{}\nWHERE /* condition */;\n\n",
                sets.join(",\n")
            ));
        }
        cx.write_to_clipboard(ClipboardItem::new_string(sql));
    }

    /// Export current result set as DELETE statements to clipboard.
    fn export_delete(&self, cx: &mut Context<Self>) {
        let ResultState::Success { columns, rows, .. } = self.active_state() else {
            return;
        };

        let mut sql = String::new();
        for row in rows {
            let where_parts: Vec<String> = columns
                .iter()
                .enumerate()
                .map(|(i, c)| match &row[i] {
                    CellValue::Null => format!("\"{}\" IS NULL", c.name),
                    v => format!("\"{}\" = '{}'", c.name, v.display().replace('\'', "''")),
                })
                .collect();
            sql.push_str(&format!(
                "DELETE FROM your_table\nWHERE\n    {};\n\n",
                where_parts.join("\n    AND ")
            ));
        }
        cx.write_to_clipboard(ClipboardItem::new_string(sql));
    }

    /// Return rows sorted by the currently selected column, or in original
    /// order when no sort column is active.
    fn sorted_rows(&self, rows: &[Vec<CellValue>]) -> Vec<Vec<CellValue>> {
        let tab = &self.tabs[self.active_tab];
        let Some(col_idx) = tab.sort_column else {
            return rows.to_vec();
        };
        let ascending = tab.sort_ascending;
        let mut sorted = rows.to_vec();
        sorted.sort_by(|a, b| {
            let cell_a = a.get(col_idx).map(|c| c.display()).unwrap_or_default();
            let cell_b = b.get(col_idx).map(|c| c.display()).unwrap_or_default();

            // Try numeric comparison first
            if let (Ok(na), Ok(nb)) = (cell_a.parse::<f64>(), cell_b.parse::<f64>()) {
                let cmp = na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal);
                return if ascending { cmp } else { cmp.reverse() };
            }

            // Fall back to string comparison
            let cmp = cell_a.cmp(&cell_b);
            if ascending { cmp } else { cmp.reverse() }
        });
        sorted
    }

    /// Copy all visible results as tab-separated values to the clipboard.
    fn copy_results_tsv(&self, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        let ResultState::Success { columns, rows, .. } = self.active_state() else {
            return;
        };

        let sorted = self.sorted_rows(rows);

        let mut tsv = String::new();
        // Header
        tsv.push_str(
            &columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join("\t"),
        );
        tsv.push('\n');
        // Rows
        for row in &sorted {
            tsv.push_str(
                &row.iter()
                    .map(|c| c.display())
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
            tsv.push('\n');
        }

        cx.write_to_clipboard(ClipboardItem::new_string(tsv));
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h(px(30.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().tab_bar_background)
            .overflow_hidden()
            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                let is_active = i == self.active_tab;
                let bg = if is_active {
                    cx.theme().colors().tab_active_background
                } else {
                    cx.theme().colors().tab_inactive_background
                };
                let idx = i;
                // Truncate label to 25 chars for preview
                let display_label: String = if tab.label.len() > 25 {
                    format!("{}...", &tab.label[..25])
                } else {
                    tab.label.clone()
                };

                h_flex()
                    .id(SharedString::from(format!("result-tab-{i}")))
                    .h_full()
                    .px_2()
                    .items_center()
                    .gap_1()
                    .bg(bg)
                    .cursor_pointer()
                    .when(is_active, |s| {
                        s.border_b_2().border_color(cx.theme().colors().text_accent)
                    })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.active_tab = idx;
                        this.selected_row = None;
                        cx.notify();
                    }))
                    .child(
                        Label::new(display_label)
                            .size(LabelSize::XSmall)
                            .color(if is_active {
                                Color::Default
                            } else {
                                Color::Muted
                            }),
                    )
                    .child(
                        IconButton::new(
                            SharedString::from(format!("close-result-tab-{i}")),
                            IconName::XCircle,
                        )
                        .icon_size(IconSize::XSmall)
                        .icon_color(Color::Muted)
                        .style(ButtonStyle::Subtle)
                        .on_click(cx.listener(
                            move |this, _, _window, cx| {
                                this.close_tab(idx, cx);
                            },
                        )),
                    )
            }))
    }

    fn render_empty(&self, _cx: &Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::ListTree)
                            .size(IconSize::Medium)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("No query results")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("Execute a query with Cmd+Enter to see results here")
                            .size(LabelSize::XSmall)
                            .color(Color::Disabled),
                    ),
            )
    }

    fn render_loading(&self, _cx: &Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::LoadCircle)
                            .size(IconSize::Medium)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("Executing query...")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            )
    }

    fn render_error(&self, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = &self.tabs[self.active_tab];
        let sql_preview = tab.sql.as_ref().map(|s| {
            let preview: String = s.chars().take(200).collect();
            if s.len() > 200 {
                format!("{preview}...")
            } else {
                preview
            }
        });

        let suggestion = if message.contains("42P01") {
            Some("Table not found. Check the table name and schema.")
        } else if message.contains("42703") {
            Some("Column not found. Check column names.")
        } else if message.contains("42601") {
            Some("Syntax error. Check your SQL syntax.")
        } else if message.contains("28P01") || message.contains("authentication") {
            Some("Authentication failed. Check username and password.")
        } else if message.contains("08") {
            Some("Connection issue. The database may be unreachable.")
        } else if message.contains("23505") {
            Some("Unique constraint violation. A duplicate value exists.")
        } else if message.contains("23503") {
            Some("Foreign key violation. Referenced record doesn't exist.")
        } else {
            None
        };

        let error_text = message.to_string();

        v_flex()
            .size_full()
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::XCircle)
                            .size(IconSize::Small)
                            .color(Color::Error),
                    )
                    .child(
                        Label::new("Query Error")
                            .size(LabelSize::Default)
                            .weight(FontWeight::SEMIBOLD)
                            .color(Color::Error),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().colors().surface_background)
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        Label::new(message.to_string())
                            .size(LabelSize::Small)
                            .color(Color::Default),
                    ),
            )
            .children(suggestion.map(|s| {
                h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::Info)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(Label::new(s).size(LabelSize::XSmall).color(Color::Muted))
            }))
            .children(sql_preview.map(|sql| {
                div()
                    .mt_1()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().colors().editor_background)
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .child(Label::new(sql).size(LabelSize::XSmall).color(Color::Muted))
            }))
            .child(
                h_flex().mt_1().child(
                    Button::new("copy-error", "Copy Error")
                        .style(ButtonStyle::Subtle)
                        .label_size(LabelSize::XSmall)
                        .on_click({
                            let err = error_text.clone();
                            cx.listener(move |_this, _, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(err.clone()));
                            })
                        }),
                ),
            )
    }

    fn render_ddl(&self, ddl: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let ddl_text = ddl.to_string();
        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().title_bar_background)
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Icon::new(IconName::FileCode)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .child(
                                Label::new("Table DDL")
                                    .size(LabelSize::Small)
                                    .weight(FontWeight::SEMIBOLD),
                            ),
                    )
                    .child(
                        IconButton::new("copy-ddl", IconName::Copy)
                            .icon_size(IconSize::XSmall)
                            .icon_color(Color::Muted)
                            .style(ButtonStyle::Subtle)
                            .tooltip(Tooltip::text("Copy DDL"))
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                if let ResultState::Ddl(ref ddl) = this.tabs[this.active_tab].state
                                {
                                    cx.write_to_clipboard(ClipboardItem::new_string(ddl.clone()));
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .id("ddl-scroll-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(cx.theme().colors().text)
                            .whitespace_nowrap()
                            .child(ddl_text),
                    ),
            )
    }

    fn render_explain(&self, plan: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let plan_owned = plan.to_string();
        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                h_flex()
                    .h(px(30.))
                    .px_2()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().title_bar_background)
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Icon::new(IconName::PlayFilled)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .child(
                                Label::new("Query Execution Plan")
                                    .size(LabelSize::Small)
                                    .weight(FontWeight::SEMIBOLD),
                            ),
                    )
                    .child(
                        IconButton::new("copy-plan", IconName::Copy)
                            .icon_size(IconSize::XSmall)
                            .icon_color(Color::Muted)
                            .style(ButtonStyle::Subtle)
                            .tooltip(Tooltip::text("Copy Plan"))
                            .on_click(cx.listener(move |_this, _, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    plan_owned.clone(),
                                ));
                            })),
                    ),
            )
            .child(
                div()
                    .id("explain-scroll-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(cx.theme().colors().text)
                            .whitespace_nowrap()
                            .child(plan.to_string()),
                    ),
            )
    }

    fn render_results(
        &self,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        duration_ms: u128,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let columns_clone = columns.to_vec();
        let sorted = self.sorted_rows(rows);
        let row_count = rows.len();
        let selected_row = self.selected_row;

        // Extract theme colors upfront so the closure captures owned Hsla values
        let surface_bg = cx.theme().colors().surface_background;
        let element_active = cx.theme().colors().element_active;
        let text_disabled = cx.theme().colors().text_disabled;
        let text_color = cx.theme().colors().text;
        let selection_bg = cx.theme().colors().element_selected;

        // Capture a weak entity for click handling inside uniform_list closure
        let this = cx.entity().downgrade();

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Header row
            .child(self.render_column_headers(columns, cx))
            // Data rows (uniform_list handles its own scrolling)
            .child(
                uniform_list("result-rows", row_count, move |range, _window, _cx| {
                    sorted[range.clone()]
                        .iter()
                        .enumerate()
                        .map(|(local_idx, row)| {
                            let idx = range.start + local_idx;
                            let this = this.clone();
                            Self::render_data_row_static(
                                idx,
                                row,
                                &columns_clone,
                                selected_row,
                                surface_bg,
                                element_active,
                                selection_bg,
                                text_disabled,
                                text_color,
                                Some(this),
                            )
                        })
                        .collect()
                })
                .flex_1(),
            )
            // Footer
            .child(self.render_footer(row_count, duration_ms, cx))
    }

    fn render_column_headers(
        &self,
        columns: &[ColumnMeta],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut header = h_flex()
            .h(px(26.))
            .px_1()
            .bg(cx.theme().colors().title_bar_background)
            .border_b_1()
            .border_color(cx.theme().colors().border);

        let hover_bg = cx.theme().colors().element_hover;

        // Row number column
        header = header.child(
            div()
                .w(px(40.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .child(
                    Label::new("#")
                        .size(LabelSize::XSmall)
                        .color(Color::Disabled),
                ),
        );

        let active_sort_column = self.tabs[self.active_tab].sort_column;
        let active_sort_ascending = self.tabs[self.active_tab].sort_ascending;

        for (i, col) in columns.iter().enumerate() {
            let col_idx = i;
            let is_sorted = active_sort_column == Some(i);
            let sort_indicator = if is_sorted {
                if active_sort_ascending { " ^" } else { " v" }
            } else {
                ""
            };

            header = header.child(
                div()
                    .id(SharedString::from(format!("col-header-{i}")))
                    .min_w(px(100.))
                    .max_w(px(200.))
                    .flex_1()
                    .px_1()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        let tab = &mut this.tabs[this.active_tab];
                        if tab.sort_column == Some(col_idx) {
                            tab.sort_ascending = !tab.sort_ascending;
                        } else {
                            tab.sort_column = Some(col_idx);
                            tab.sort_ascending = true;
                        }
                        cx.notify();
                    }))
                    .child(
                        Label::new(format!("{}{sort_indicator}", col.name))
                            .size(LabelSize::XSmall)
                            .weight(FontWeight::SEMIBOLD)
                            .truncate(),
                    ),
            );
        }

        header
    }

    #[allow(clippy::too_many_arguments)]
    fn render_data_row_static(
        idx: usize,
        row: &[CellValue],
        _columns: &[ColumnMeta],
        selected_row: Option<usize>,
        surface_bg: Hsla,
        element_active: Hsla,
        selection_bg: Hsla,
        text_disabled: Hsla,
        text_color: Hsla,
        weak_entity: Option<WeakEntity<ResultPanel>>,
    ) -> Stateful<Div> {
        let is_selected = selected_row == Some(idx);
        let bg = if is_selected {
            selection_bg
        } else if idx.is_multiple_of(2) {
            surface_bg
        } else {
            // Odd rows get no explicit bg (transparent)
            Hsla::default()
        };

        let mut row_div = div()
            .id(ElementId::Name(SharedString::from(format!("row-{idx}"))))
            .flex()
            .flex_row()
            .h(px(22.))
            .px_1()
            .cursor_pointer()
            .when(is_selected, |s| s.bg(bg))
            .when(!is_selected && idx.is_multiple_of(2), |s| s.bg(surface_bg))
            .hover(|s| s.bg(element_active));

        // Add click handler if we have a weak entity reference
        if let Some(this) = weak_entity {
            row_div = row_div.on_click(move |_event, _window, cx| {
                this.update(cx, |panel, cx| {
                    panel.select_row(idx, cx);
                })
                .ok();
            });
        }

        // Row number
        row_div = row_div.child(
            div()
                .w(px(40.))
                .flex_shrink_0()
                .text_xs()
                .text_color(text_disabled)
                .flex()
                .items_center()
                .child(format!("{}", idx + 1)),
        );

        for cell in row.iter() {
            let display = cell.display();
            let is_null = cell.is_null();

            row_div = row_div.child(
                div()
                    .min_w(px(100.))
                    .max_w(px(200.))
                    .flex_1()
                    .px_1()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_xs()
                            .overflow_hidden()
                            .text_ellipsis()
                            .when(is_null, |s| s.text_color(text_disabled))
                            .when(!is_null, |s| s.text_color(text_color))
                            .child(display),
                    ),
            );
        }

        row_div
    }

    fn render_footer(
        &self,
        row_count: usize,
        duration_ms: u128,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let row_word = if row_count == 1 { "row" } else { "rows" };
        let has_selected = self.selected_row.is_some();
        let has_more = self
            .tabs
            .get(self.active_tab)
            .map(|t| t.has_more)
            .unwrap_or(false);

        // Left: row count + timing (indicate truncation when has_more)
        let left_info = if has_more {
            format!("{row_count}+ rows (limited) in {duration_ms}ms")
        } else {
            format!("{row_count} {row_word} in {duration_ms}ms")
        };
        // Center: tab indicator
        let center_info = format!("Tab {} / {}", self.active_tab + 1, self.tabs.len());
        // Selected row info
        let selected_info = self
            .selected_row
            .map(|r| format!("Row {}", r + 1))
            .unwrap_or_default();

        let mut footer = h_flex()
            .items_center()
            .h(px(30.))
            .px_2()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().title_bar_background);

        // Left side: row count + timing
        footer = footer.child(
            h_flex()
                .flex_1()
                .gap_2()
                .child(
                    Label::new(left_info)
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                )
                .when(!selected_info.is_empty(), |s| {
                    s.child(
                        Label::new(selected_info)
                            .size(LabelSize::XSmall)
                            .color(Color::Accent),
                    )
                }),
        );

        // Center: tab info
        footer = footer.child(
            Label::new(center_info)
                .size(LabelSize::XSmall)
                .color(Color::Disabled),
        );

        // Load More / Load All buttons when results are truncated
        if has_more {
            footer = footer.child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("load-more", "Load 500 more")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .on_click(cx.listener(|this, _, _window, cx| this.load_more(cx))),
                    )
                    .child(
                        Button::new("load-all", "Load All")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .on_click(cx.listener(|this, _, _window, cx| this.load_all(cx))),
                    ),
            );
        }

        // Right side: action buttons
        let mut buttons = h_flex().flex_1().justify_end().gap_0p5();

        // Show Edit Row and Delete Row buttons when a row is selected
        if has_selected {
            buttons = buttons
                .child(
                    IconButton::new("edit-row-btn", IconName::Pencil)
                        .icon_size(IconSize::XSmall)
                        .icon_color(Color::Muted)
                        .style(ButtonStyle::Subtle)
                        .tooltip(Tooltip::text("Edit Row"))
                        .on_click(cx.listener(|this, _, _window, cx| this.edit_selected_row(cx))),
                )
                .child(
                    IconButton::new("delete-row-btn", IconName::Trash)
                        .icon_size(IconSize::XSmall)
                        .icon_color(Color::Error)
                        .style(ButtonStyle::Subtle)
                        .tooltip(Tooltip::text("Delete Row"))
                        .on_click(cx.listener(|this, _, _window, cx| this.delete_selected_row(cx))),
                );
        }

        buttons = buttons
            .child(
                IconButton::new("refresh-btn", IconName::RefreshTitle)
                    .icon_size(IconSize::XSmall)
                    .icon_color(Color::Muted)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("Refresh"))
                    .on_click(cx.listener(|this, _, _window, cx| this.refresh_current_tab(cx))),
            )
            .child(
                IconButton::new("clear-btn", IconName::Eraser)
                    .icon_size(IconSize::XSmall)
                    .icon_color(Color::Muted)
                    .style(ButtonStyle::Subtle)
                    .tooltip(Tooltip::text("Clear All Tabs"))
                    .on_click(cx.listener(|this, _, _window, cx| this.clear_all(cx))),
            )
            .child(
                Button::new("export-csv", "CSV")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .tooltip(Tooltip::text("Copy as CSV"))
                    .on_click(cx.listener(|this, _, _window, cx| this.export_csv(cx))),
            )
            .child(
                Button::new("export-json", "JSON")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .tooltip(Tooltip::text("Copy as JSON"))
                    .on_click(cx.listener(|this, _, _window, cx| this.export_json(cx))),
            )
            .child(
                Button::new("export-insert", "INS")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .tooltip(Tooltip::text("Copy as INSERT"))
                    .on_click(cx.listener(|this, _, _window, cx| this.export_insert(cx))),
            )
            .child(
                Button::new("export-update", "UPD")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .tooltip(Tooltip::text("Copy as UPDATE"))
                    .on_click(cx.listener(|this, _, _window, cx| this.export_update(cx))),
            )
            .child(
                Button::new("export-delete", "DEL")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::XSmall)
                    .tooltip(Tooltip::text("Copy as DELETE"))
                    .on_click(cx.listener(|this, _, _window, cx| this.export_delete(cx))),
            );

        footer = footer.child(buttons);
        footer
    }
}

impl Focusable for ResultPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PanelEvent> for ResultPanel {}

impl Render for ResultPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .id("result-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "c" && event.keystroke.modifiers.platform {
                    this.copy_results_tsv(cx);
                }
            }));

        // Handle empty state (no tabs)
        if self.tabs.is_empty() {
            return panel.child(self.render_empty(cx));
        }

        // Tab bar shown when there are tabs (always, so close button is accessible)
        panel = panel.child(self.render_tab_bar(cx));

        // Clone state data needed for rendering to avoid borrow issues
        let state = self.tabs[self.active_tab].state.clone();
        let content = match state {
            ResultState::Loading => self.render_loading(cx).into_any_element(),
            ResultState::Error(msg) => self.render_error(&msg, cx).into_any_element(),
            ResultState::Ddl(ddl) => self.render_ddl(&ddl, cx).into_any_element(),
            ResultState::Explain(plan) => self.render_explain(&plan, cx).into_any_element(),
            ResultState::Success {
                columns,
                rows,
                duration_ms,
            } => self
                .render_results(&columns, &rows, duration_ms, cx)
                .into_any_element(),
        };

        panel.child(content)
    }
}

impl Panel for ResultPanel {
    fn persistent_name() -> &'static str {
        "ResultPanel"
    }

    fn panel_key() -> &'static str {
        "ResultPanel"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Bottom
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Bottom | DockPosition::Right)
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn default_size(&self, _window: &Window, _cx: &App) -> Pixels {
        px(300.)
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<ui::IconName> {
        Some(IconName::ListTree)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Query Results")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleFocus)
    }

    fn activation_priority(&self) -> u32 {
        5
    }

    fn starts_open(&self, _window: &Window, _cx: &App) -> bool {
        false
    }

    fn set_active(&mut self, active: bool, _window: &mut Window, _cx: &mut Context<Self>) {
        self.active = active;
    }
}

/// Parse EXPLAIN (FORMAT JSON) output and produce a human-readable tree.
fn format_explain_plan(json_text: &str) -> String {
    let Ok(plan) = serde_json::from_str::<serde_json::Value>(json_text) else {
        // Not valid JSON — return as-is (plain text EXPLAIN output)
        return json_text.to_string();
    };

    let mut output = String::new();

    // PostgreSQL EXPLAIN JSON returns an array with one element
    let plan_array = if let Some(arr) = plan.as_array() {
        arr
    } else {
        return json_text.to_string();
    };

    for plan_entry in plan_array {
        if let Some(plan_node) = plan_entry.get("Plan") {
            format_plan_node(plan_node, 0, &mut output);
        }

        // Add summary
        if let Some(planning_time) = plan_entry.get("Planning Time") {
            output.push_str(&format!(
                "\nPlanning Time: {:.3} ms\n",
                planning_time.as_f64().unwrap_or(0.0)
            ));
        }
        if let Some(execution_time) = plan_entry.get("Execution Time") {
            output.push_str(&format!(
                "Execution Time: {:.3} ms\n",
                execution_time.as_f64().unwrap_or(0.0)
            ));
        }
    }

    output
}

fn format_plan_node(node: &serde_json::Value, depth: usize, output: &mut String) {
    let indent = "  ".repeat(depth);

    let node_type = node
        .get("Node Type")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let relation = node
        .get("Relation Name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let alias = node.get("Alias").and_then(|v| v.as_str()).unwrap_or("");

    let startup_cost = node
        .get("Startup Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let total_cost = node
        .get("Total Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let actual_startup = node
        .get("Actual Startup Time")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let actual_total = node
        .get("Actual Total Time")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let rows = node
        .get("Actual Rows")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let loops = node
        .get("Actual Loops")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    let relation_info = if !relation.is_empty() {
        if !alias.is_empty() && alias != relation {
            format!(" on {} ({})", relation, alias)
        } else {
            format!(" on {}", relation)
        }
    } else {
        String::new()
    };

    output.push_str(&format!(
        "{indent}-> {node_type}{relation_info}  (cost={startup_cost:.2}..{total_cost:.2} rows={rows} loops={loops})\n"
    ));
    output.push_str(&format!(
        "{indent}   Actual: {actual_startup:.3}..{actual_total:.3} ms\n"
    ));

    // Filter
    if let Some(filter) = node.get("Filter").and_then(|v| v.as_str()) {
        output.push_str(&format!("{indent}   Filter: {filter}\n"));
    }

    // Join conditions
    if let Some(cond) = node.get("Hash Cond").and_then(|v| v.as_str()) {
        output.push_str(&format!("{indent}   Hash Cond: {cond}\n"));
    }
    if let Some(cond) = node.get("Join Filter").and_then(|v| v.as_str()) {
        output.push_str(&format!("{indent}   Join Filter: {cond}\n"));
    }

    // Sort key
    if let Some(sort_key) = node.get("Sort Key").and_then(|v| v.as_array()) {
        let keys: Vec<&str> = sort_key.iter().filter_map(|v| v.as_str()).collect();
        output.push_str(&format!("{indent}   Sort Key: {}\n", keys.join(", ")));
    }

    // Shared buffers
    if let Some(hit) = node.get("Shared Hit Blocks").and_then(|v| v.as_i64()) {
        let read = node
            .get("Shared Read Blocks")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if hit > 0 || read > 0 {
            output.push_str(&format!(
                "{indent}   Buffers: shared hit={hit} read={read}\n"
            ));
        }
    }

    // Child plans
    if let Some(plans) = node.get("Plans").and_then(|v| v.as_array()) {
        for child in plans {
            format_plan_node(child, depth + 1, output);
        }
    }
}
