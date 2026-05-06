use std::sync::Arc;

use editor::Editor;
use gpui::*;
use picker::{Picker, PickerDelegate};
use ui::prelude::*;
use workspace::Workspace;

use pgblade_core::schema::SchemaTree;

/// The kind of SQL completion item, used for display labels and sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Table,
    Column,
    Function,
    Keyword,
    Schema,
    Sequence,
}

impl CompletionKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Table => "TBL",
            Self::Column => "COL",
            Self::Function => "FN",
            Self::Keyword => "KW",
            Self::Schema => "SCH",
            Self::Sequence => "SEQ",
        }
    }
}

/// A single entry in the SQL completion picker.
#[derive(Debug, Clone)]
pub struct SqlCompletionItem {
    pub label: String,
    pub detail: String,
    pub insert_text: String,
    pub kind: CompletionKind,
}

/// Delegate that drives the SQL completion picker behavior.
pub struct SqlCompletionDelegate {
    items: Vec<SqlCompletionItem>,
    filtered: Vec<usize>,
    selected: usize,
    workspace: WeakEntity<Workspace>,
}

impl SqlCompletionDelegate {
    pub fn new(items: Vec<SqlCompletionItem>, workspace: WeakEntity<Workspace>) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        Self {
            items,
            filtered,
            selected: 0,
            workspace,
        }
    }
}

impl PickerDelegate for SqlCompletionDelegate {
    type ListItem = ui::ListItem;

    fn match_count(&self) -> usize {
        self.filtered.len()
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(
        &mut self,
        ix: usize,
        _window: &mut Window,
        _cx: &mut Context<Picker<Self>>,
    ) {
        self.selected = ix;
    }

    fn placeholder_text(&self, _window: &mut Window, _cx: &mut App) -> Arc<str> {
        "Search tables, columns, functions...".into()
    }

    fn update_matches(
        &mut self,
        query: String,
        _window: &mut Window,
        _cx: &mut Context<Picker<Self>>,
    ) -> Task<()> {
        let query_lower = query.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                query_lower.is_empty()
                    || item.label.to_lowercase().contains(&query_lower)
                    || item.insert_text.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, window: &mut Window, cx: &mut Context<Picker<Self>>) {
        if let Some(&idx) = self.filtered.get(self.selected) {
            let text = self.items[idx].insert_text.clone();

            if let Some(workspace) = self.workspace.upgrade() {
                workspace.update(cx, |workspace, cx| {
                    if let Some(active_item) = workspace.active_item(cx)
                        && let Some(editor) = active_item.act_as::<Editor>(cx)
                    {
                        editor.update(cx, |editor, _cx| {
                            editor.insert(&text, window, _cx);
                        });
                    }
                });
            }
        }
        cx.emit(DismissEvent);
    }

    fn dismissed(&mut self, _window: &mut Window, _cx: &mut Context<Picker<Self>>) {}

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        _window: &mut Window,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let &item_idx = self.filtered.get(ix)?;
        let item = &self.items[item_idx];

        let kind_label = item.kind.label().to_string();
        let label_text = item.label.clone();
        let detail_text = item.detail.clone();

        Some(
            ui::ListItem::new(SharedString::from(format!("completion-{ix}")))
                .inset(true)
                .spacing(ui::ListItemSpacing::Sparse)
                .toggle_state(selected)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .w(px(30.))
                                .text_xs()
                                .text_color(cx.theme().colors().text_disabled)
                                .child(kind_label),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(ui::Label::new(label_text).size(ui::LabelSize::Small)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().colors().text_muted)
                                .child(detail_text),
                        ),
                ),
        )
    }
}

/// Build completion items from the introspected schema tree.
///
/// Includes SQL keywords, schema names, tables, columns, views, functions, and sequences.
pub fn build_completion_items(schema_tree: &SchemaTree) -> Vec<SqlCompletionItem> {
    let mut items = Vec::new();

    // SQL keywords
    let keywords = [
        "SELECT",
        "FROM",
        "WHERE",
        "AND",
        "OR",
        "NOT",
        "IN",
        "JOIN",
        "LEFT",
        "RIGHT",
        "INNER",
        "OUTER",
        "ON",
        "GROUP BY",
        "ORDER BY",
        "HAVING",
        "LIMIT",
        "OFFSET",
        "INSERT INTO",
        "VALUES",
        "UPDATE",
        "SET",
        "DELETE FROM",
        "CREATE TABLE",
        "ALTER TABLE",
        "DROP TABLE",
        "CREATE INDEX",
        "AS",
        "DISTINCT",
        "BETWEEN",
        "LIKE",
        "ILIKE",
        "IS NULL",
        "IS NOT NULL",
        "CASE",
        "WHEN",
        "THEN",
        "ELSE",
        "END",
        "EXISTS",
        "WITH",
        "UNION",
        "INTERSECT",
        "EXCEPT",
        "RETURNING",
        "COUNT",
        "SUM",
        "AVG",
        "MIN",
        "MAX",
        "COALESCE",
        "NULLIF",
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
        "EXPLAIN ANALYZE",
    ];

    for kw in &keywords {
        items.push(SqlCompletionItem {
            label: kw.to_string(),
            detail: "keyword".to_string(),
            insert_text: kw.to_string(),
            kind: CompletionKind::Keyword,
        });
    }

    // Schema objects from the introspected tree
    for schema in &schema_tree.schemas {
        // Schema itself
        items.push(SqlCompletionItem {
            label: schema.info.name.clone(),
            detail: "schema".to_string(),
            insert_text: format!("\"{}\"", schema.info.name),
            kind: CompletionKind::Schema,
        });

        // Tables
        for table in &schema.tables {
            // Unqualified name
            items.push(SqlCompletionItem {
                label: table.info.name.clone(),
                detail: format!("table ({})", schema.info.name),
                insert_text: table.info.name.clone(),
                kind: CompletionKind::Table,
            });

            // Qualified name
            items.push(SqlCompletionItem {
                label: format!("{}.{}", schema.info.name, table.info.name),
                detail: "table".to_string(),
                insert_text: format!("\"{}\".\"{}\"", schema.info.name, table.info.name),
                kind: CompletionKind::Table,
            });

            // Columns
            for col in &table.columns {
                let pk = if col.is_primary_key { " PK" } else { "" };
                items.push(SqlCompletionItem {
                    label: format!("{}.{}", table.info.name, col.name),
                    detail: format!("{}{}", col.data_type, pk),
                    insert_text: col.name.clone(),
                    kind: CompletionKind::Column,
                });
            }
        }

        // Views
        for view in &schema.views {
            items.push(SqlCompletionItem {
                label: view.info.name.clone(),
                detail: format!("view ({})", schema.info.name),
                insert_text: view.info.name.clone(),
                kind: CompletionKind::Table,
            });
        }

        // Materialized views
        for mv in &schema.materialized_views {
            items.push(SqlCompletionItem {
                label: mv.info.name.clone(),
                detail: format!("materialized view ({})", schema.info.name),
                insert_text: mv.info.name.clone(),
                kind: CompletionKind::Table,
            });
        }

        // Functions
        for func in &schema.functions {
            items.push(SqlCompletionItem {
                label: func.name.clone(),
                detail: format!("({}) -> {}", func.arguments, func.return_type),
                insert_text: format!("{}()", func.name),
                kind: CompletionKind::Function,
            });
        }

        // Sequences
        for seq in &schema.sequences {
            items.push(SqlCompletionItem {
                label: seq.name.clone(),
                detail: "sequence".to_string(),
                insert_text: seq.name.clone(),
                kind: CompletionKind::Sequence,
            });
        }
    }

    items
}
