//! The embedded terminal view: draws a `TermSession`'s frames as monospace
//! rows of styled runs, sends input to it, scrolls its history, and resizes
//! the shell to fit.
//!
//! Input: printable text (including IME composition and dead keys) arrives
//! through GPUI's input handler; special keys and ctrl combinations go
//! through Ghostty's key encoder. Mouse: drag selects, double click a word,
//! triple click a line, ⌘-click opens a link; when a program tracks the
//! mouse (vim, htop, Claude's menus) clicks, drags and the wheel go to it
//! instead (hold ⇧ to select anyway). ⌘C copies, ⌘V pastes, ⌘F searches the
//! scrollback.

use std::cell::Cell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    Bounds, ClipboardItem, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    FontWeight, IntoElement, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent, SharedString, Task,
    UTF16Selection, Window, canvas, div, font, point, prelude::*, px, rgb, size,
};
use libghostty_vt::key::Mods;

use crate::term::{self, Frame, Geometry, KeyInput, MouseKind, TermSession};
use crate::theme;

/// Defaults when the theme names no font (Ghostty's `font-family`).
const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 13.0;
/// Line height as a multiple of the font size.
const LINE_SPACING: f32 = 1.3;

/// Keys that are never text, whatever GPUI reports as their `key_char`.
const SPECIAL_KEYS: &[&str] = &[
    "enter",
    "tab",
    "escape",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "insert",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

/// Whether a keystroke types text (handled by the input handler, which also
/// covers IME and dead keys) rather than being a key the encoder sends.
/// Option-modified keys type their character (Option-2 is `@` on many
/// layouts); GPUI does not report which Option key is held, so Option is not
/// treated as Alt.
pub fn is_text_input(keystroke: &Keystroke) -> bool {
    let mods = &keystroke.modifiers;
    !mods.control
        && !mods.platform
        && !SPECIAL_KEYS.contains(&keystroke.key.as_str())
        && keystroke
            .key_char
            .as_deref()
            .is_some_and(|t| !t.is_empty() && !t.chars().any(char::is_control))
}

fn mods_of(m: &Modifiers) -> Mods {
    let mut mods = Mods::empty();
    mods.set(Mods::SHIFT, m.shift);
    mods.set(Mods::ALT, m.alt);
    mods.set(Mods::CTRL, m.control);
    mods.set(Mods::SUPER, m.platform);
    mods
}

fn button_of(button: MouseButton) -> Option<term::MouseButton> {
    match button {
        MouseButton::Left => Some(term::MouseButton::Left),
        MouseButton::Middle => Some(term::MouseButton::Middle),
        MouseButton::Right => Some(term::MouseButton::Right),
        _ => None,
    }
}

/// ⌘F: matching lines in the scrollback and the one shown.
struct Search {
    query: String,
    hits: Vec<(usize, String)>,
    current: usize,
}

pub struct TerminalView {
    pub cwd: PathBuf,
    session: Result<TermSession, String>,
    pub focus: FocusHandle,
    /// The view's size from the last paint, for fitting the grid.
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cell_width: Option<f32>,
    font: SharedString,
    font_size: f32,
    line_height: f32,
    cursor_bar: bool,
    /// The theme generation applied; a newer one re-applies colors and font.
    theme_generation: Option<u64>,
    exited: bool,
    /// A mouse drag is selecting.
    dragging: bool,
    /// A button pressed while a program tracks the mouse (for drag motion).
    reporting_button: Option<term::MouseButton>,
    /// IME composition in progress, drawn at the cursor until committed.
    marked: Option<String>,
    /// The cursor cell from the last frame (for the IME candidate window).
    last_cursor: Option<(u16, u16)>,
    search: Option<Search>,
    /// The tmux session this view is attached to, when tmux-backed.
    pub tmux_session: Option<String>,
    /// The last frame built from the engine; rebuilt only when something
    /// changed (output, input, scrolling, selection, size, theme), not on
    /// every redraw of the window.
    frame_cache: Option<Rc<Frame>>,
    frame_dirty: bool,
    _pump: Task<()>,
}

impl TerminalView {
    /// A login shell in `cwd`; `command`, when given, is typed into it once
    /// it starts (so the shell stays when the command ends).
    pub fn new(cwd: PathBuf, command: Option<String>, cx: &mut Context<Self>) -> Self {
        let mut view = Self::with_session(cwd, None, cx);
        if let (Ok(session), Some(command)) = (&mut view.session, command) {
            session.write(format!("{command}\r").as_bytes());
        }
        view
    }

    /// A client attached to a tmux session on the app's server. The session
    /// outlives this view: quitting the app only detaches.
    pub fn tmux(cwd: PathBuf, session: String, cx: &mut Context<Self>) -> Self {
        Self::with_session(cwd, Some(session), cx)
    }

    fn spawn(cwd: &std::path::Path, tmux_session: Option<&str>) -> Result<TermSession, String> {
        match tmux_session {
            Some(name) => {
                let argv = crate::tmux::attach_argv(&crate::tmux::Server::fubbik(), name);
                TermSession::spawn_argv(cwd, 100, 30, Some(&argv))
            }
            None => TermSession::spawn(cwd, 100, 30),
        }
    }

    fn with_session(cwd: PathBuf, tmux_session: Option<String>, cx: &mut Context<Self>) -> Self {
        let session = Self::spawn(&cwd, tmux_session.as_deref());
        let mut view = Self {
            cwd,
            session,
            focus: cx.focus_handle(),
            bounds: Rc::new(Cell::new(None)),
            cell_width: None,
            font: FONT_FAMILY.into(),
            font_size: FONT_SIZE,
            line_height: (FONT_SIZE * LINE_SPACING).round(),
            cursor_bar: false,
            theme_generation: None,
            exited: false,
            dragging: false,
            reporting_button: None,
            marked: None,
            last_cursor: None,
            search: None,
            tmux_session,
            frame_cache: None,
            frame_dirty: true,
            _pump: Task::ready(()),
        };
        view.start_pump(cx);
        view
    }

    pub fn title(&self) -> String {
        self.session
            .as_ref()
            .map(TermSession::title)
            .unwrap_or_default()
    }

    /// After the shell (or tmux client) ends: reattach while the tmux
    /// session lives, else a fresh login shell.
    fn restart(&mut self, cx: &mut Context<Self>) {
        self.frame_dirty = true;
        let server = crate::tmux::Server::fubbik();
        if let Some(name) = &self.tmux_session
            && !crate::tmux::exists(&server, name)
        {
            self.tmux_session = None;
        }
        self.session = Self::spawn(&self.cwd, self.tmux_session.as_deref());
        self.exited = false;
        self.theme_generation = None; // the new session needs the theme's colors
        self.start_pump(cx);
    }

    /// Drains output whenever the PTY reader signals it; ends (after a last
    /// drain that notices the exit) when the PTY closes.
    fn start_pump(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = &self.session else { return };
        let wake = session.wake();
        self._pump = cx.spawn(async move |this, cx| {
            while wake.recv().await.is_ok() {
                if this.update(cx, |view, cx| view.tick(cx)).is_err() {
                    return;
                }
            }
            this.update(cx, |view, cx| view.tick(cx)).ok();
        });
    }

    /// Fits the grid to the size the view was last painted at.
    fn fit(&mut self) {
        let (Ok(session), Some(bounds), Some(cell_w)) =
            (&mut self.session, self.bounds.get(), self.cell_width)
        else {
            return;
        };
        let cols = (f32::from(bounds.size.width) / cell_w).floor() as u16;
        let rows = (f32::from(bounds.size.height) / self.line_height).floor() as u16;
        if cols > 1 && rows > 1 && (cols, rows) != session.size() {
            session.resize(cols, rows, cell_w.round() as u32, self.line_height as u32);
            self.frame_dirty = true;
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = &mut self.session else {
            return;
        };
        let mut changed = session.pump();
        if !self.exited && session.exited() {
            self.exited = true;
            changed = true;
        }
        if changed {
            self.frame_dirty = true;
            cx.notify();
        }
    }

    fn geometry(&self) -> Geometry {
        Geometry {
            cell_width: self.cell_width.unwrap_or(8.0).round() as u32,
            cell_height: self.line_height as u32,
        }
    }

    /// A window position relative to the grid's top-left, in pixels.
    fn local(&self, position: Point<Pixels>) -> Option<(f32, f32)> {
        let bounds = self.bounds.get()?;
        Some((
            f32::from(position.x - bounds.origin.x).max(0.),
            f32::from(position.y - bounds.origin.y).max(0.),
        ))
    }

    /// The viewport cell under a window position.
    fn cell_at(&self, position: Point<Pixels>) -> Option<(u16, u16)> {
        let (x, y) = self.local(position)?;
        let cell_w = self.cell_width?;
        let (cols, rows) = self.session.as_ref().ok()?.size();
        Some((
            ((x / cell_w) as u16).min(cols.saturating_sub(1)),
            ((y / self.line_height) as u16).min(rows.saturating_sub(1)),
        ))
    }

    // ── keys ───────────────────────────────────────────────────────────

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.frame_dirty = true;
        let keystroke = &event.keystroke;
        let mods = &keystroke.modifiers;
        if self.search.is_some() && self.on_search_key(keystroke, cx) {
            cx.stop_propagation();
            return;
        }
        if self.exited && keystroke.key == "enter" {
            self.restart(cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let Ok(session) = &mut self.session else {
            return;
        };
        if mods.platform {
            // ⌘C copies a selection, ⌘V pastes, ⌘F searches; other ⌘ keys
            // belong to the app (⌘K, ⌘1…).
            match keystroke.key.as_str() {
                "c" => {
                    if let Some(text) = session.selected_text() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        cx.stop_propagation();
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        session.paste(&text);
                        cx.stop_propagation();
                    }
                }
                "f" => {
                    self.search = Some(Search {
                        query: String::new(),
                        hits: Vec::new(),
                        current: 0,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }
                _ => {}
            }
            return;
        }
        session.clear_selection();
        if is_text_input(keystroke) {
            // Left for the input handler (`replace_text_in_range`), which
            // also sees IME composition and dead keys.
            return;
        }
        session.key(&KeyInput {
            key: keystroke.key.clone(),
            text: keystroke.key_char.clone(),
            shift: mods.shift,
            alt: mods.alt,
            ctrl: mods.control,
            cmd: false,
        });
        cx.stop_propagation();
    }

    /// Typed text from the input handler goes to the shell as-is.
    fn type_text(&mut self, text: &str) {
        if let Ok(session) = &mut self.session {
            session.scroll_to_bottom();
            session.write(text.as_bytes());
        }
    }

    // ── search ─────────────────────────────────────────────────────────

    /// Keys while the search bar is open. True when consumed.
    fn on_search_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(search) = &mut self.search else {
            return false;
        };
        // ⌘V pastes into the query, not into the shell behind it.
        if keystroke.modifiers.platform && keystroke.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                search
                    .query
                    .push_str(text.lines().next().unwrap_or_default());
                self.run_search();
            }
            return true;
        }
        match keystroke.key.as_str() {
            "escape" => self.search = None,
            "enter" if !search.hits.is_empty() => {
                let n = search.hits.len();
                search.current = if keystroke.modifiers.shift {
                    (search.current + n - 1) % n
                } else {
                    (search.current + 1) % n
                };
                self.show_hit();
            }
            "backspace" => {
                search.query.pop();
                self.run_search();
            }
            _ if is_text_input(keystroke) => {
                search
                    .query
                    .push_str(keystroke.key_char.as_deref().unwrap_or_default());
                self.run_search();
            }
            _ => return keystroke.modifiers.platform && keystroke.key == "f",
        }
        cx.notify();
        true
    }

    fn run_search(&mut self) {
        let (Ok(session), Some(search)) = (&self.session, &mut self.search) else {
            return;
        };
        search.hits = session.search(&search.query);
        // Start from the newest match, nearest the prompt.
        search.current = search.hits.len().saturating_sub(1);
        self.show_hit();
    }

    /// Scrolls the current match to the middle of the screen.
    fn show_hit(&mut self) {
        let (Ok(session), Some(search)) = (&mut self.session, &self.search) else {
            return;
        };
        if let Some((row, _)) = search.hits.get(search.current) {
            let half = session.size().1 as usize / 2;
            session.scroll_to_row(row.saturating_sub(half));
        }
    }

    // ── mouse ──────────────────────────────────────────────────────────

    /// Whether the mouse goes to the program instead of selecting.
    fn reporting(&self, modifiers: &Modifiers) -> bool {
        !modifiers.shift && self.session.as_ref().is_ok_and(TermSession::mouse_tracking)
    }

    fn report(
        &mut self,
        kind: MouseKind,
        button: Option<term::MouseButton>,
        position: Point<Pixels>,
        modifiers: &Modifiers,
    ) {
        let Some((x, y)) = self.local(position) else {
            return;
        };
        let geometry = self.geometry();
        if let Ok(session) = &mut self.session {
            session.mouse(kind, button, x, y, mods_of(modifiers), geometry);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.frame_dirty = true;
        self.focus.focus(window);
        if self.reporting(&event.modifiers) {
            let button = button_of(event.button);
            self.reporting_button = button;
            self.report(MouseKind::Press, button, event.position, &event.modifiers);
            return;
        }
        if event.button != MouseButton::Left {
            return;
        }
        let Some((col, row)) = self.cell_at(event.position) else {
            return;
        };
        let Ok(session) = &mut self.session else {
            return;
        };
        if event.modifiers.platform {
            let frame = session.frame();
            if let Some(url) = frame
                .lines
                .get(row as usize)
                .and_then(|line| term::url_at(line, col as usize))
            {
                cx.open_url(&url);
            }
            return;
        }
        match event.click_count {
            1 => {
                session.select_start(col, row);
                self.dragging = true;
            }
            2 => session.select_word(col, row),
            _ => session.select_line(col, row),
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.frame_dirty = true;
        if let Some(button) = self.reporting_button {
            self.report(
                MouseKind::Motion,
                Some(button),
                event.position,
                &event.modifiers,
            );
            return;
        }
        if !self.dragging || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let Some((col, row)) = self.cell_at(event.position) else {
            return;
        };
        if let Ok(session) = &mut self.session {
            session.select_extend(col, row);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.frame_dirty = true;
        if let Some(button) = self.reporting_button.take() {
            self.report(
                MouseKind::Release,
                Some(button),
                event.position,
                &event.modifiers,
            );
            return;
        }
        self.dragging = false;
        if let Ok(session) = &mut self.session {
            session.select_end();
        }
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.frame_dirty = true;
        let lines = match event.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / self.line_height,
        };
        let rows = lines.round() as isize;
        if rows == 0 {
            return;
        }
        if self.reporting(&event.modifiers) {
            let button = if rows > 0 {
                term::MouseButton::WheelUp
            } else {
                term::MouseButton::WheelDown
            };
            for _ in 0..rows.unsigned_abs().min(10) {
                self.report(
                    MouseKind::Press,
                    Some(button),
                    event.position,
                    &event.modifiers,
                );
            }
            return;
        }
        // History lives in tmux for a tmux-backed view: scroll its copy mode.
        if let Some(name) = &self.tmux_session {
            let _ = crate::tmux::scroll(&crate::tmux::Server::fubbik(), name, rows);
            return;
        }
        let Ok(session) = &mut self.session else {
            return;
        };
        // GPUI: positive is up; Ghostty: negative is up.
        session.scroll(-rows);
        cx.notify();
    }

    /// The search bar's query and match count, when open.
    #[cfg(test)]
    pub fn search_status(&self) -> Option<(String, usize)> {
        self.search
            .as_ref()
            .map(|s| (s.query.clone(), s.hits.len()))
    }

    #[cfg(test)]
    pub fn screen_text(&mut self) -> String {
        let Ok(session) = &mut self.session else {
            return String::new();
        };
        session.pump();
        let frame = session.frame();
        frame
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|r| r.text.as_str())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// ⇧⌘K: empties the screen and scrollback; the shell redraws its prompt.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if let Some(name) = &self.tmux_session {
            let _ = crate::tmux::clear(&crate::tmux::Server::fubbik(), name);
        }
        if let Ok(session) = &mut self.session {
            session.clear();
        }
        self.frame_dirty = true;
        cx.notify();
    }

    /// Copies the selection, or the whole visible screen without one.
    pub fn copy_screen(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = &mut self.session else {
            return;
        };
        if let Some(text) = session.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            return;
        }
        let frame = session.frame();
        let text: Vec<String> = frame
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|r| r.text.as_str())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        cx.write_to_clipboard(ClipboardItem::new_string(
            text.join("\n").trim_end().to_string(),
        ));
    }
}

/// IME and plain typing: GPUI hands committed text to `replace_text_in_range`
/// and in-progress composition to `replace_and_mark_text_in_range`.
impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let marked: Vec<u16> = self.marked.as_deref()?.encode_utf16().collect();
        let range = range.start.min(marked.len())..range.end.min(marked.len());
        *adjusted = Some(range.clone());
        String::from_utf16(&marked[range]).ok()
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self
            .marked
            .as_deref()
            .map_or(0, |m| m.encode_utf16().count());
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_deref().map(|m| 0..m.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.frame_dirty = true;
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.frame_dirty = true;
        self.marked = None;
        if self.search.is_none() && !text.is_empty() {
            self.type_text(text);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.frame_dirty = true;
        self.marked = (!text.is_empty()).then(|| text.to_string());
        cx.notify();
    }

    /// Where the IME candidate window goes: at the cursor.
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (col, row) = self.last_cursor?;
        let cell_w = self.cell_width?;
        Some(Bounds::new(
            point(
                element.origin.x + px(col as f32 * cell_w),
                element.origin.y + px(row as f32 * self.line_height),
            ),
            size(px(cell_w), px(self.line_height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn row_element(
    frame: &Frame,
    index: usize,
    default_bg: Option<u32>,
    line_height: f32,
    cell_w: f32,
) -> impl IntoElement {
    let runs = frame.rows[index].iter().map(|run| {
        // Exactly the run's columns, so wide or fallback glyphs cannot shift
        // the rest of the line.
        let mut span = div()
            .flex_none()
            .w(px(run.cells as f32 * cell_w))
            .overflow_hidden()
            .text_color(rgb(run.fg.hex()))
            .child(SharedString::from(run.text.clone()));
        if let Some(bg) = run.bg.map(|c| c.hex()).filter(|bg| Some(*bg) != default_bg) {
            span = span.bg(rgb(bg));
        }
        if run.bold {
            span = span.font_weight(FontWeight::BOLD);
        }
        if run.italic {
            span = span.italic();
        }
        if run.underline {
            span = span.underline();
        }
        if run.strikethrough {
            span = span.line_through();
        }
        span
    });
    div()
        .flex()
        .h(px(line_height))
        .whitespace_nowrap()
        .children(runs)
}

impl TerminalView {
    fn render_search(&self) -> Option<impl IntoElement> {
        let search = self.search.as_ref()?;
        let count = if search.query.is_empty() {
            "type to search the scrollback".to_string()
        } else if search.hits.is_empty() {
            "no matches".to_string()
        } else {
            format!("{}/{}", search.current + 1, search.hits.len())
        };
        let note = self
            .tmux_session
            .as_ref()
            .map(|_| " · tmux tab: searches what is on screen");
        Some(
            div()
                .absolute()
                .top_1()
                .right_2()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(theme::accent())
                .bg(theme::elevated())
                .shadow(theme::frame_shadow())
                .text_xs()
                .child(format!("Find: {}▏", search.query))
                .child(div().text_color(theme::text_muted()).child(format!(
                    "{count}{}  ⏎ next · ⇧⏎ previous · esc",
                    note.unwrap_or("")
                ))),
        )
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A new theme: colors into the engine, font and cursor from it.
        if self.theme_generation != Some(theme::generation()) {
            self.theme_generation = Some(theme::generation());
            let t = theme::terminal();
            if let Ok(session) = &mut self.session {
                session.set_colors(&t.colors);
            }
            self.font = t.font_family.unwrap_or_else(|| FONT_FAMILY.into()).into();
            self.font_size = t.font_size.unwrap_or(FONT_SIZE);
            self.line_height = (self.font_size * LINE_SPACING).round();
            self.cursor_bar = t.cursor_bar;
            self.cell_width = None;
            self.frame_dirty = true;
        }
        if self.cell_width.is_none() {
            let text_system = window.text_system();
            // A theme's font that is not installed: use the default monospace
            // rather than GPUI's (proportional) fallback, which breaks columns.
            if self.font.as_ref() != FONT_FAMILY
                && !text_system
                    .all_font_names()
                    .iter()
                    .any(|name| *name == self.font.as_ref())
            {
                self.font = FONT_FAMILY.into();
            }
            let font_id = text_system.resolve_font(&font(self.font.clone()));
            self.cell_width = text_system
                .advance(font_id, px(self.font_size), 'm')
                .ok()
                .map(|size| f32::from(size.width));
        }
        self.fit();
        let line_h = self.line_height;
        let cursor_bar = self.cursor_bar;
        let focused = self.focus.is_focused(window);
        let cell_w = self.cell_width.unwrap_or(8.0);
        let bounds = self.bounds.clone();
        let (entity, focus) = (cx.entity(), self.focus.clone());
        // Records the painted size (a change asks for one more frame, in
        // which `fit` resizes the grid) and registers the input handler.
        let measure = canvas(
            move |b, window, _| {
                if bounds.get() != Some(b) {
                    bounds.set(Some(b));
                    window.refresh();
                }
                b
            },
            move |_, b, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(b, entity), cx);
            },
        )
        .absolute()
        .size_full();

        let marked = self.marked.clone();
        let body = match &mut self.session {
            Err(error) => div()
                .p_4()
                .text_color(theme::danger())
                .child(format!("Could not start the shell: {error}"))
                .into_any_element(),
            Ok(session) => {
                let frame = match &self.frame_cache {
                    Some(frame) if !self.frame_dirty => frame.clone(),
                    _ => {
                        let frame = Rc::new(session.frame());
                        self.frame_cache = Some(frame.clone());
                        self.frame_dirty = false;
                        frame
                    }
                };
                self.last_cursor = frame.cursor;
                // On glass, cells with the terminal's default background stay see-through.
                let default_bg = frame.background.map(|c| c.hex());
                let cursor_color: gpui::Hsla = match frame.cursor_color {
                    Some(c) => rgb(c.hex()).into(),
                    None => theme::accent(),
                };
                let cursor = frame.cursor.map(|(x, y)| {
                    let at = div()
                        .absolute()
                        .left(px(x as f32 * cell_w))
                        .top(px(y as f32 * line_h))
                        .h(px(line_h));
                    if let Some(text) = &marked {
                        // IME composition: shown underlined until committed.
                        return at
                            .px_0p5()
                            .bg(theme::elevated())
                            .border_b_1()
                            .border_color(cursor_color)
                            .whitespace_nowrap()
                            .child(text.clone());
                    }
                    match (focused, cursor_bar) {
                        (true, true) => at.w(px(2.)).bg(cursor_color),
                        (true, false) => at.w(px(cell_w)).bg(cursor_color.opacity(0.6)),
                        (false, _) => at.w(px(cell_w)).border_1().border_color(cursor_color),
                    }
                });
                div()
                    .relative()
                    .children(
                        (0..frame.rows.len())
                            .map(|i| row_element(&frame, i, default_bg, line_h, cell_w)),
                    )
                    .children(cursor)
                    .into_any_element()
            }
        };

        div()
            .id("terminal")
            .key_context("Terminal")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .font_family(self.font.clone())
            .text_size(px(self.font_size))
            .line_height(px(line_h))
            .text_color(theme::text())
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(measure)
            .child(body)
            .children(self.render_search())
            .when(self.exited, |d| {
                d.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .right_2()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(theme::elevated())
                        .text_xs()
                        .child("Shell exited — press Enter to restart"),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            modifiers,
            key: key.into(),
            key_char: key_char.map(String::from),
        }
    }

    #[test]
    fn text_goes_to_the_input_handler_and_keys_to_the_encoder() {
        // Given
        let none = Modifiers::default();
        let ctrl = Modifiers {
            control: true,
            ..Default::default()
        };
        let alt = Modifiers {
            alt: true,
            ..Default::default()
        };

        // When
        let cases = [
            is_text_input(&stroke("a", Some("a"), none)),
            is_text_input(&stroke("2", Some("@"), alt)),
            is_text_input(&stroke("c", Some("c"), ctrl)),
            is_text_input(&stroke("enter", Some("\n"), none)),
            is_text_input(&stroke("left", None, none)),
            is_text_input(&stroke("space", Some(" "), none)),
        ];

        // Then
        assert_eq!(cases, [true, true, false, false, false, true]);
    }
}
