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
}

impl ResultPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            active: false,
            state: ResultState::Empty,
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
        let rows_clone = rows.to_vec();
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
                    rows_clone[range.clone()]
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
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let mut header = div()
            .flex()
            .flex_row()
            .h(px(24.))
            .px_1()
            .bg(cx.theme().colors().title_bar_background)
            .border_b_1()
            .border_color(cx.theme().colors().border);

        // Row number column
        header = header.child(
            div()
                .w(px(40.))
                .flex_shrink_0()
                .text_xs()
                .text_color(cx.theme().colors().text_disabled)
                .flex()
                .items_center()
                .child("#"),
        );

        for col in columns {
            header = header.child(
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
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(col.name.clone()),
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
