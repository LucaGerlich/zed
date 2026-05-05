use std::sync::Arc;

use gpui::*;
use tokio::runtime::Runtime;
use ui::prelude::*;
use ui::{Button, ButtonStyle, IconName, LabelSize};
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
    Empty,
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

pub struct ResultPanel {
    focus_handle: FocusHandle,
    active: bool,
    state: ResultState,
    sort_column: Option<usize>,
    sort_ascending: bool,
}

impl ResultPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            active: false,
            state: ResultState::Empty,
            sort_column: None,
            sort_ascending: true,
        }
    }

    /// Execute a SQL query using the given session and display results.
    pub fn execute_query(
        &mut self,
        sql: String,
        session: Arc<dyn DatabaseSession>,
        runtime: Arc<Runtime>,
        cx: &mut Context<Self>,
    ) {
        self.state = ResultState::Loading;
        self.sort_column = None;
        self.sort_ascending = true;
        cx.notify();

        let started = std::time::Instant::now();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql).await })
                .await;

            let elapsed = started.elapsed().as_millis();

            this.update(cx, |panel, cx| {
                match result {
                    Ok(Ok(result_set)) => {
                        panel.state = ResultState::Success {
                            columns: result_set.columns,
                            rows: result_set.rows,
                            duration_ms: elapsed,
                        };
                    }
                    Ok(Err(e)) => {
                        panel.state = ResultState::Error(e.to_string());
                    }
                    Err(e) => {
                        panel.state = ResultState::Error(format!("runtime error: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Display DDL text in the result panel.
    pub fn show_ddl(&mut self, ddl: String, cx: &mut Context<Self>) {
        self.state = ResultState::Ddl(ddl);
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
        self.state = ResultState::Loading;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn(async move { session.execute(&sql).await })
                .await;

            this.update(cx, |panel, cx| {
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
                        panel.state = ResultState::Explain(formatted);
                    }
                    Ok(Err(e)) => {
                        panel.state = ResultState::Error(e.to_string());
                    }
                    Err(e) => {
                        panel.state = ResultState::Error(format!("runtime error: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Export current result set as CSV to clipboard.
    fn export_csv(&self, cx: &mut Context<Self>) {
        let ResultState::Success { columns, rows, .. } = &self.state else {
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
        let ResultState::Success { columns, rows, .. } = &self.state else {
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
        let ResultState::Success { columns, rows, .. } = &self.state else {
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

    /// Return rows sorted by the currently selected column, or in original
    /// order when no sort column is active.
    fn sorted_rows(&self, rows: &[Vec<CellValue>]) -> Vec<Vec<CellValue>> {
        let Some(col_idx) = self.sort_column else {
            return rows.to_vec();
        };
        let ascending = self.sort_ascending;
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
        let ResultState::Success { columns, rows, .. } = &self.state else {
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

    fn render_empty(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().colors().text_muted)
                            .child("No query results"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors().text_disabled)
                            .child("Execute a query with Cmd+Enter to see results here"),
                    ),
            )
    }

    fn render_loading(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().colors().text_muted)
                    .child("Executing query..."),
            )
    }

    fn render_error(&self, message: &str, cx: &Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(gpui::red())
                    .child("Query Error"),
            )
            .child(
                div()
                    .mt_2()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().colors().surface_background)
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors().text)
                            .child(message.to_string()),
                    ),
            )
    }

    fn render_ddl(&self, ddl: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let ddl_text = ddl.to_string();
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().title_bar_background)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .child("Table DDL"),
                    )
                    .child(
                        Button::new("copy-ddl", "Copy")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                if let ResultState::Ddl(ref ddl) = this.state {
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
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().title_bar_background)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .child("Query Execution Plan"),
                    )
                    .child(
                        Button::new("copy-plan", "Copy")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
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

        // Extract theme colors upfront so the closure captures owned Hsla values
        let surface_bg = cx.theme().colors().surface_background;
        let element_active = cx.theme().colors().element_active;
        let text_disabled = cx.theme().colors().text_disabled;
        let text_color = cx.theme().colors().text;

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
                            Self::render_data_row_static(
                                idx,
                                row,
                                &columns_clone,
                                surface_bg,
                                element_active,
                                text_disabled,
                                text_color,
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
        let mut header = div()
            .flex()
            .flex_row()
            .h(px(24.))
            .px_1()
            .bg(cx.theme().colors().title_bar_background)
            .border_b_1()
            .border_color(cx.theme().colors().border);

        // Extract theme colors upfront so closures capture owned Hsla values
        let text_disabled = cx.theme().colors().text_disabled;
        let text_color = cx.theme().colors().text;
        let hover_bg = cx.theme().colors().element_hover;

        // Row number column
        header = header.child(
            div()
                .w(px(40.))
                .flex_shrink_0()
                .text_xs()
                .text_color(text_disabled)
                .flex()
                .items_center()
                .child("#"),
        );

        for (i, col) in columns.iter().enumerate() {
            let col_idx = i;
            let is_sorted = self.sort_column == Some(i);
            let sort_indicator = if is_sorted {
                if self.sort_ascending { " ^" } else { " v" }
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
                        if this.sort_column == Some(col_idx) {
                            this.sort_ascending = !this.sort_ascending;
                        } else {
                            this.sort_column = Some(col_idx);
                            this.sort_ascending = true;
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(format!("{}{sort_indicator}", col.name)),
                    ),
            );
        }

        header
    }

    fn render_data_row_static(
        idx: usize,
        row: &[CellValue],
        _columns: &[ColumnMeta],
        surface_bg: Hsla,
        element_active: Hsla,
        text_disabled: Hsla,
        text_color: Hsla,
    ) -> Stateful<Div> {
        let is_even = idx.is_multiple_of(2);

        let mut row_div = div()
            .id(ElementId::Name(SharedString::from(format!("row-{idx}"))))
            .flex()
            .flex_row()
            .h(px(22.))
            .px_1()
            .when(is_even, |s| s.bg(surface_bg))
            .hover(|s| s.bg(element_active));

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
        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(28.))
            .px_2()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().title_bar_background)
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child(format!("{row_count} {row_word} in {duration_ms}ms")),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        Button::new("export-csv", "CSV")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .on_click(cx.listener(|this, _, _window, cx| this.export_csv(cx))),
                    )
                    .child(
                        Button::new("export-json", "JSON")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .on_click(cx.listener(|this, _, _window, cx| this.export_json(cx))),
                    )
                    .child(
                        Button::new("export-insert", "INSERT")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::XSmall)
                            .on_click(cx.listener(|this, _, _window, cx| this.export_insert(cx))),
                    ),
            )
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
        // Clone state data needed for rendering to avoid borrow issues
        let state = self.state.clone();
        let content = match state {
            ResultState::Empty => self.render_empty(cx).into_any_element(),
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

        div()
            .id("result-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "c" && event.keystroke.modifiers.platform {
                    this.copy_results_tsv(cx);
                }
            }))
            .child(content)
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
