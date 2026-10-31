//! The embedded terminal view: draws a `TermSession`'s frames as monospace
//! rows of styled runs, sends keys and pastes to it, scrolls its history,
//! and resizes the shell to fit. Mouse: drag selects, double click a word,
//! triple click a line; ⌘C copies the selection.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Bounds, ClipboardItem, Context, FocusHandle, FontWeight, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, SharedString, Task, Window, canvas, div, font, prelude::*, px, rgb,
};

use crate::term::{Frame, KeyInput, TermSession};
use crate::theme;

/// Defaults when the theme names no font (Ghostty's `font-family`).
const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 13.0;
/// Line height as a multiple of the font size.
const LINE_SPACING: f32 = 1.3;
/// Shell output is drained this often while the view exists.
const PUMP_EVERY: Duration = Duration::from_millis(16);

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
    /// The tmux session this view is attached to, when tmux-backed.
    pub tmux_session: Option<String>,
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
        let pump = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PUMP_EVERY).await;
                if this.update(cx, |view, cx| view.tick(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
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
            tmux_session,
            _pump: pump,
        }
    }

    pub fn title(&self) -> String {
        self.session
            .as_ref()
            .map(TermSession::title)
            .unwrap_or_default()
    }

    /// After the shell (or tmux client) ends: reattach while the tmux
    /// session lives, else a fresh login shell.
    fn restart(&mut self) {
        let server = crate::tmux::Server::fubbik();
        if let Some(name) = &self.tmux_session
            && !crate::tmux::exists(&server, name)
        {
            self.tmux_session = None;
        }
        self.session = Self::spawn(&self.cwd, self.tmux_session.as_deref());
        self.exited = false;
        self.theme_generation = None; // the new session needs the theme's colors
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = &mut self.session else {
            return;
        };
        let mut changed = session.pump();
        if let (Some(bounds), Some(cell_w)) = (self.bounds.get(), self.cell_width) {
            let cols = (f32::from(bounds.size.width) / cell_w).floor() as u16;
            let rows = (f32::from(bounds.size.height) / self.line_height).floor() as u16;
            if cols > 1 && rows > 1 && (cols, rows) != session.size() {
                session.resize(cols, rows, cell_w.round() as u32, self.line_height as u32);
                changed = true;
            }
        }
        if !self.exited && session.exited() {
            self.exited = true;
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let mods = &keystroke.modifiers;
        if self.exited && keystroke.key == "enter" {
            self.restart();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let Ok(session) = &mut self.session else {
            return;
        };
        if mods.platform {
            // ⌘C copies a selection, ⌘V pastes; other ⌘ keys belong to the app (⌘K, ⌘1…).
            if keystroke.key == "c"
                && let Some(text) = session.selected_text()
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
                return;
            }
            if keystroke.key == "v"
                && let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
            {
                session.paste(&text);
                cx.stop_propagation();
            }
            return;
        }
        session.clear_selection();
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

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Ok(session) = &mut self.session else {
            return;
        };
        let lines = match event.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / self.line_height,
        };
        let rows = lines.round() as isize;
        if rows == 0 {
            return;
        }
        // History lives in tmux for a tmux-backed view: scroll its copy mode.
        if let Some(name) = &self.tmux_session {
            let _ = crate::tmux::scroll(&crate::tmux::Server::fubbik(), name, rows);
            return;
        }
        // GPUI: positive is up; Ghostty: negative is up.
        session.scroll(-rows);
        cx.notify();
    }

    /// The viewport cell under a window position.
    fn cell_at(&self, position: Point<Pixels>) -> Option<(u16, u16)> {
        let bounds = self.bounds.get()?;
        let cell_w = self.cell_width?;
        let (cols, rows) = self.session.as_ref().ok()?.size();
        let x = f32::from(position.x - bounds.origin.x) / cell_w;
        let y = f32::from(position.y - bounds.origin.y) / self.line_height;
        Some((
            (x.max(0.0) as u16).min(cols.saturating_sub(1)),
            (y.max(0.0) as u16).min(rows.saturating_sub(1)),
        ))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window);
        let Some((col, row)) = self.cell_at(event.position) else {
            return;
        };
        let Ok(session) = &mut self.session else {
            return;
        };
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

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.dragging = false;
        if let Ok(session) = &mut self.session {
            session.select_end();
        }
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

fn row_element(
    frame: &Frame,
    index: usize,
    default_bg: Option<u32>,
    line_height: f32,
) -> impl IntoElement {
    let runs = frame.rows[index].iter().map(|run| {
        let mut span = div()
            .flex_none()
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
        let line_h = self.line_height;
        let cursor_bar = self.cursor_bar;
        let focused = self.focus.is_focused(window);
        let cell_w = self.cell_width.unwrap_or(8.0);
        let bounds = self.bounds.clone();
        let measure = canvas(move |b, _, _| bounds.set(Some(b)), |_, _, _, _| {})
            .absolute()
            .size_full();

        let body = match &mut self.session {
            Err(error) => div()
                .p_4()
                .text_color(theme::danger())
                .child(format!("Could not start the shell: {error}"))
                .into_any_element(),
            Ok(session) => {
                let frame = session.frame();
                // On glass, cells with the terminal's default background stay see-through.
                let default_bg = frame.background.map(|c| c.hex());
                let cursor_color: gpui::Hsla = match frame.cursor_color {
                    Some(c) => rgb(c.hex()).into(),
                    None => theme::accent(),
                };
                let cursor = frame.cursor.map(|(x, y)| {
                    let block = div()
                        .absolute()
                        .left(px(x as f32 * cell_w))
                        .top(px(y as f32 * line_h))
                        .h(px(line_h));
                    match (focused, cursor_bar) {
                        (true, true) => block.w(px(2.)).bg(cursor_color),
                        (true, false) => block.w(px(cell_w)).bg(cursor_color.opacity(0.6)),
                        (false, _) => block.w(px(cell_w)).border_1().border_color(cursor_color),
                    }
                });
                div()
                    .relative()
                    .children(
                        (0..frame.rows.len()).map(|i| row_element(&frame, i, default_bg, line_h)),
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
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(measure)
            .child(body)
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
