use std::cell::RefCell;
use std::rc::Rc;

use editor::CompletionContext;
use editor::CompletionProvider;
use editor::Editor;
use gpui::{Context, Entity, Task, Window};
use language::{Buffer, CodeLabel};
use project::{Completion, CompletionDisplayOptions, CompletionResponse, CompletionSource};

use text::ToOffset;

use crate::sql_completion::SqlCompletionItem;

/// A completion provider that supplies SQL schema-based completions
/// while delegating to any existing inner provider (typically the LSP-based
/// Project provider) to preserve language server completions for non-SQL files.
pub struct SqlCompletionProvider {
    inner: RefCell<Option<Rc<dyn CompletionProvider>>>,
    items: RefCell<Vec<SqlCompletionItem>>,
}

impl Default for SqlCompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlCompletionProvider {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(None),
            items: RefCell::new(Vec::new()),
        }
    }

    /// Replace the current SQL completion items with a new set derived from
    /// the introspected database schema.
    pub fn set_items(&self, items: Vec<SqlCompletionItem>) {
        *self.items.borrow_mut() = items;
    }

    /// Store the original completion provider so we can delegate to it.
    pub fn set_inner(&self, provider: Option<Rc<dyn CompletionProvider>>) {
        *self.inner.borrow_mut() = provider;
    }
}

impl CompletionProvider for SqlCompletionProvider {
    fn completions(
        &self,
        buffer: &Entity<Buffer>,
        buffer_position: text::Anchor,
        trigger: CompletionContext,
        window: &mut Window,
        cx: &mut Context<Editor>,
    ) -> Task<anyhow::Result<Vec<CompletionResponse>>> {
        // Delegate to the inner provider first (LSP completions).
        let inner_task = if let Some(inner) = self.inner.borrow().as_ref() {
            inner.completions(buffer, buffer_position, trigger.clone(), window, cx)
        } else {
            Task::ready(Ok(Vec::new()))
        };

        // Build our SQL completions synchronously.
        let items = self.items.borrow();
        if items.is_empty() {
            return inner_task;
        }

        let buf = buffer.read(cx);
        let offset = buffer_position.to_offset(buf);
        let snapshot = buf.text();
        let before_cursor = &snapshot[..offset.min(snapshot.len())];

        let word_start = before_cursor
            .rfind(|c: char| c.is_whitespace() || "(),;.\"'".contains(c))
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &before_cursor[word_start..];

        if prefix.len() < 2 {
            return inner_task;
        }

        let prefix_lower = prefix.to_lowercase();

        let sql_completions: Vec<Completion> = items
            .iter()
            .filter(|item| {
                item.label.to_lowercase().starts_with(&prefix_lower)
                    || item.insert_text.to_lowercase().starts_with(&prefix_lower)
            })
            .take(20)
            .map(|item| {
                let word_start_anchor = buf.anchor_before(word_start);
                let label_text = format!("{} \u{2014} {}", item.label, item.detail);
                let filter_len = item.label.len();

                Completion {
                    replace_range: word_start_anchor..buffer_position,
                    new_text: item.insert_text.clone(),
                    label: CodeLabel::plain(label_text, Some(&item.label[..filter_len])),
                    documentation: None,
                    source: CompletionSource::Custom,
                    icon_path: None,
                    match_start: Some(word_start_anchor),
                    snippet_deduplication_key: None,
                    insert_text_mode: None,
                    confirm: None,
                }
            })
            .collect();

        if sql_completions.is_empty() {
            return inner_task;
        }

        let sql_response = CompletionResponse {
            completions: sql_completions,
            display_options: CompletionDisplayOptions::default(),
            is_incomplete: false,
        };

        // Merge inner provider results with our SQL results.
        cx.foreground_executor().spawn(async move {
            let mut responses = inner_task.await.unwrap_or_default();
            responses.push(sql_response);
            Ok(responses)
        })
    }

    fn is_completion_trigger(
        &self,
        buffer: &Entity<Buffer>,
        position: language::Anchor,
        text: &str,
        trigger_in_words: bool,
        cx: &mut Context<Editor>,
    ) -> bool {
        // Delegate to inner provider first.
        let inner_trigger = if let Some(inner) = self.inner.borrow().as_ref() {
            inner.is_completion_trigger(buffer, position, text, trigger_in_words, cx)
        } else {
            false
        };

        // Also trigger on dot, underscore, or alphanumeric input when we have SQL items loaded.
        let has_items = !self.items.borrow().is_empty();
        let sql_trigger = has_items && (trigger_in_words || text == "." || text == "_");

        inner_trigger || sql_trigger
    }

    fn resolve_completions(
        &self,
        buffer: Entity<Buffer>,
        completion_indices: Vec<usize>,
        completions: Rc<RefCell<Box<[Completion]>>>,
        cx: &mut Context<Editor>,
    ) -> Task<anyhow::Result<bool>> {
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.resolve_completions(buffer, completion_indices, completions, cx)
        } else {
            Task::ready(Ok(false))
        }
    }

    fn apply_additional_edits_for_completion(
        &self,
        buffer: Entity<Buffer>,
        completions: Rc<RefCell<Box<[Completion]>>>,
        completion_index: usize,
        push_to_history: bool,
        all_commit_ranges: Vec<std::ops::Range<language::Anchor>>,
        cx: &mut Context<Editor>,
    ) -> Task<anyhow::Result<Option<language::Transaction>>> {
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.apply_additional_edits_for_completion(
                buffer,
                completions,
                completion_index,
                push_to_history,
                all_commit_ranges,
                cx,
            )
        } else {
            Task::ready(Ok(None))
        }
    }

    fn sort_completions(&self) -> bool {
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.sort_completions()
        } else {
            true
        }
    }

    fn filter_completions(&self) -> bool {
        if let Some(inner) = self.inner.borrow().as_ref() {
            inner.filter_completions()
        } else {
            true
        }
    }
}
