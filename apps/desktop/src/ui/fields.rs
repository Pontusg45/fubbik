//! Text fields: key handling, and what pressing enter in each one does.

use super::command_palette::PALETTE_ROWS;
use super::*;

impl Workspace {
    /// Saves the capture note; false when it could not start (no title, no
    /// space), so the capture box stays open.
    pub(super) fn submit_note(&mut self, cx: &mut Context<Self>) -> bool {
        let title = self.note_title.text.trim().to_string();
        if title.is_empty() {
            self.show_error("A note needs a title");
            return false;
        }
        let Some(space_id) = self
            .repos
            .get(self.selected)
            .and_then(|v| v.knowledge.as_ref())
            .and_then(|k| k.as_ref().ok())
            .and_then(|k| k.space.as_ref())
            .map(|s| s.id.clone())
        else {
            self.show_error("This repo has no fubbik space");
            return false;
        };
        let content = self.note_body.text.clone();
        let client = self.client.clone();
        let task = cx
            .background_executor()
            .spawn(async move { client.quick_note(&space_id, &title, &content) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        // Cleared only once saved; a failure keeps the text.
                        this.note_title.text.clear();
                        this.note_body.text.clear();
                        this.show_info("Note saved to fubbik")
                    }
                    Err(error) => this.show_error(format!("Could not save the note: {error}")),
                }
                this.poll_now(true, cx);
            })
            .ok();
        })
        .detach();
        true
    }

    pub(super) fn submit_group(&mut self) {
        let group = self.group_field.text.trim().to_string();
        let Some(view) = self.repos.get_mut(self.selected) else {
            return;
        };
        view.entry.group = (!group.is_empty()).then_some(group);
        self.save_config();
        self.show_info("Group saved");
    }

    pub(super) fn field_mut(&mut self, field: Field) -> &mut TextField {
        match field {
            Field::Palette => &mut self.palette_field,
            Field::LogFilter => &mut self.log_filter,
            Field::ScriptFilter => &mut self.script_filter,
            Field::NoteTitle => &mut self.note_title,
            Field::NoteBody => &mut self.note_body,
            Field::Group => &mut self.group_field,
            Field::NewWorktree => &mut self.worktree_field,
            Field::Reply => &mut self.reply_field,
            Field::FubbikUrl => &mut self.fubbik_url_field,
            Field::WebUrl => &mut self.web_url_field,
            Field::FubbikBin => &mut self.fubbik_bin_field,
            Field::Commit => &mut self.commit_field,
            Field::GraphSearch => &mut self.graph_search,
            Field::BranchFilter => &mut self.branch_filter,
        }
    }

    pub(super) fn on_field_key(
        &mut self,
        field: Field,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        // The run prompt answers keys even when a field had focus.
        if self.run_prompt.is_some() || self.confirm.is_some() {
            return;
        }
        if keystroke.modifiers.platform && keystroke.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.field_mut(field)
                    .text
                    .push_str(text.lines().next().unwrap_or_default());
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        let outcome = apply_key(
            &mut self.field_mut(field).text,
            &keystroke.key,
            keystroke.key_char.as_deref(),
            &keystroke.modifiers,
        );
        if outcome == KeyOutcome::Ignored {
            return;
        }
        cx.stop_propagation();
        match (field, outcome) {
            (Field::Palette, KeyOutcome::Edited) => {
                self.palette_selected = 0;
                self.search_chunks(cx);
            }
            (Field::Palette, KeyOutcome::Up) => {
                self.palette_selected = self.palette_selected.saturating_sub(1)
            }
            (Field::Palette, KeyOutcome::Down) => {
                // Only the rows shown can be chosen.
                let last = self
                    .palette_matches()
                    .len()
                    .min(PALETTE_ROWS)
                    .saturating_sub(1);
                self.palette_selected = (self.palette_selected + 1).min(last);
            }
            (Field::Palette, KeyOutcome::Cancel) => self.toggle_palette(window),
            (Field::Palette, KeyOutcome::Submit) => {
                let matches = self.palette_matches();
                let chosen = matches
                    .get(
                        self.palette_selected
                            .min(matches.len().min(PALETTE_ROWS).saturating_sub(1)),
                    )
                    .cloned();
                self.toggle_palette(window);
                if let Some(entry) = chosen {
                    self.choose_from_palette(entry.command, window, cx);
                }
            }
            (Field::NoteTitle, KeyOutcome::Submit) => self.note_body.focus.focus(window),
            (Field::NoteBody, KeyOutcome::Submit) => {
                if self.submit_note(cx) {
                    self.capture_open = false;
                    self.focus.focus(window);
                }
            }
            (Field::Reply, KeyOutcome::Submit) => self.send_reply(cx),
            (Field::Commit, KeyOutcome::Submit) => self.commit_staged(cx),
            (Field::GraphSearch, KeyOutcome::Submit) => self.graph_fly_to_match(cx),
            (Field::GraphSearch, KeyOutcome::Cancel) => {
                self.graph_search.text.clear();
                self.focus.focus(window);
            }
            (Field::FubbikUrl | Field::WebUrl | Field::FubbikBin, KeyOutcome::Submit) => {
                self.submit_preference(field, cx);
                self.focus.focus(window);
            }
            (Field::NewWorktree, KeyOutcome::Submit) => self.create_worktree(self.selected, cx),
            (Field::Group, KeyOutcome::Submit) => {
                self.submit_group();
                self.focus.focus(window);
            }
            (Field::ScriptFilter, KeyOutcome::Submit) => self.run_first_script_match(cx),
            (Field::ScriptFilter, KeyOutcome::Cancel) => {
                self.script_filter.text.clear();
                self.focus.focus(window);
            }
            (_, KeyOutcome::Cancel) => {
                self.capture_open = false;
                self.focus.focus(window);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Wraps a field's look with focus tracking and key handling.
    pub(super) fn field(
        &self,
        field: Field,
        placeholder: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text_field = match field {
            Field::Palette => &self.palette_field,
            Field::LogFilter => &self.log_filter,
            Field::ScriptFilter => &self.script_filter,
            Field::NoteTitle => &self.note_title,
            Field::NoteBody => &self.note_body,
            Field::Group => &self.group_field,
            Field::NewWorktree => &self.worktree_field,
            Field::Reply => &self.reply_field,
            Field::FubbikUrl => &self.fubbik_url_field,
            Field::WebUrl => &self.web_url_field,
            Field::FubbikBin => &self.fubbik_bin_field,
            Field::Commit => &self.commit_field,
            Field::GraphSearch => &self.graph_search,
            Field::BranchFilter => &self.branch_filter,
        };
        let focus = text_field.focus.clone();
        widgets::text_box(&text_field.text, placeholder, focus.is_focused(window))
            .track_focus(&focus)
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
                focus.focus(window)
            })
            .on_key_down(cx.listener(move |this, event, window, cx| {
                this.on_field_key(field, event, window, cx)
            }))
    }
}
