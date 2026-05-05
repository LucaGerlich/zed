use std::sync::Arc;

use gpui::*;
use tokio::runtime::Runtime;
use ui::IconName;
use ui::prelude::*;
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

    fn render_results(
        &self,
        columns: &[ColumnMeta],
        rows: &[Vec<CellValue>],
        duration_ms: u128,
        cx: &Context<Self>,
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
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(24.))
            .px_2()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .bg(cx.theme().colors().title_bar_background)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child(format!(
                        "{} row{} returned",
                        row_count,
                        if row_count == 1 { "" } else { "s" }
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child(format!("{}ms", duration_ms)),
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
        let content = match &self.state {
            ResultState::Empty => self.render_empty(cx).into_any_element(),
            ResultState::Loading => self.render_loading(cx).into_any_element(),
            ResultState::Error(msg) => self.render_error(&msg.clone(), cx).into_any_element(),
            ResultState::Success {
                columns,
                rows,
                duration_ms,
            } => self
                .render_results(columns, rows, *duration_ms, cx)
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
