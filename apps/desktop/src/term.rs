//! An embedded terminal: a shell on a PTY (`portable-pty`) whose output feeds
//! Ghostty's VT engine (`libghostty-vt`). The UI reads plain `Frame`s of
//! styled runs and sends keys through Ghostty's key encoder.
//!
//! libghostty-vt types are `!Send`, so a `TermSession` lives on the main
//! thread; a reader thread forwards PTY output over a channel and `pump`
//! drains it.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use libghostty_vt::fmt::Format;
use libghostty_vt::key::{self, Action, Key, Mods};
use libghostty_vt::render::{CellIterator, RenderState, RowIterator};
use libghostty_vt::screen::{CellWide, TrackedGridRef};
use libghostty_vt::selection::{FormatOptions, SelectLineOptions, SelectWordOptions, Selection};
use libghostty_vt::style::{Palette as VtPalette, RgbColor};
use libghostty_vt::terminal::{Mode, Point, PointCoordinate, ScrollViewport};
use libghostty_vt::{Terminal, TerminalOptions};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

pub const SCROLLBACK: usize = 10_000;
/// Selected cells: the theme accent, darkened, under bright text.
const SELECTION_BG: Rgb = Rgb(0x3d, 0x47, 0x99);
const SELECTION_FG: Rgb = Rgb(0xff, 0xff, 0xff);

impl From<Rgb> for RgbColor {
    fn from(c: Rgb) -> Self {
        RgbColor {
            r: c.0,
            g: c.1,
            b: c.2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl From<RgbColor> for Rgb {
    fn from(c: RgbColor) -> Self {
        Rgb(c.r, c.g, c.b)
    }
}

impl Rgb {
    pub fn hex(self) -> u32 {
        (self.0 as u32) << 16 | (self.1 as u32) << 8 | self.2 as u32
    }
}

/// Consecutive cells with the same look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub fg: Rgb,
    /// None: the terminal background.
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub cols: u16,
    pub rows: Vec<Vec<Run>>,
    /// (column, row) in the viewport, when visible.
    pub cursor: Option<(u16, u16)>,
    pub background: Option<Rgb>,
    /// The theme's cursor color, when set.
    pub cursor_color: Option<Rgb>,
    pub foreground: Option<Rgb>,
}

/// A key press from the UI, in terms GPUI gives us.
#[derive(Debug, Clone, Default)]
pub struct KeyInput {
    /// GPUI's key name: "a", "enter", "left", "f5", …
    pub key: String,
    /// The text the key types, if any.
    pub text: Option<String>,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub cmd: bool,
}

/// GPUI key names → Ghostty physical keys.
pub fn map_key(name: &str) -> Key {
    let letter = |c: char| -> Key {
        match c {
            'a' => Key::A,
            'b' => Key::B,
            'c' => Key::C,
            'd' => Key::D,
            'e' => Key::E,
            'f' => Key::F,
            'g' => Key::G,
            'h' => Key::H,
            'i' => Key::I,
            'j' => Key::J,
            'k' => Key::K,
            'l' => Key::L,
            'm' => Key::M,
            'n' => Key::N,
            'o' => Key::O,
            'p' => Key::P,
            'q' => Key::Q,
            'r' => Key::R,
            's' => Key::S,
            't' => Key::T,
            'u' => Key::U,
            'v' => Key::V,
            'w' => Key::W,
            'x' => Key::X,
            'y' => Key::Y,
            'z' => Key::Z,
            '0' => Key::Digit0,
            '1' => Key::Digit1,
            '2' => Key::Digit2,
            '3' => Key::Digit3,
            '4' => Key::Digit4,
            '5' => Key::Digit5,
            '6' => Key::Digit6,
            '7' => Key::Digit7,
            '8' => Key::Digit8,
            '9' => Key::Digit9,
            '`' => Key::Backquote,
            '\\' => Key::Backslash,
            '[' => Key::BracketLeft,
            ']' => Key::BracketRight,
            ',' => Key::Comma,
            '=' => Key::Equal,
            '-' => Key::Minus,
            '.' => Key::Period,
            '\'' => Key::Quote,
            ';' => Key::Semicolon,
            '/' => Key::Slash,
            _ => Key::Unidentified,
        }
    };
    match name {
        "enter" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "escape" => Key::Escape,
        "space" => Key::Space,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "insert" => Key::Insert,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => letter(c.to_ascii_lowercase()),
                _ => Key::Unidentified,
            }
        }
    }
}

pub struct TermSession {
    terminal: Terminal<'static, 'static>,
    render: RenderState<'static>,
    row_iter: RowIterator<'static>,
    cell_iter: CellIterator<'static>,
    encoder: key::Encoder<'static>,
    event: key::Event<'static>,
    /// Bytes the terminal answers back (device status, mode queries).
    replies: Rc<RefCell<Vec<u8>>>,
    title: Rc<RefCell<String>>,
    output: Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    cols: u16,
    rows: u16,
    exited: bool,
    /// Where a mouse drag started; follows the content as it scrolls.
    drag_anchor: Option<TrackedGridRef>,
    /// Selected cells' colors (from the theme).
    selection: (Rgb, Rgb),
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl TermSession {
    /// Starts the login shell in `cwd`.
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<Self, String> {
        Self::spawn_argv(cwd, cols, rows, None)
    }

    /// Runs `argv` (a login shell when None) in `cwd` on a new PTY.
    pub fn spawn_argv(
        cwd: &Path,
        cols: u16,
        rows: u16,
        argv: Option<&[String]>,
    ) -> Result<Self, String> {
        let (cols, rows) = (cols.max(2), rows.max(2));
        let pty = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(err)?;
        let mut command = match argv {
            Some([program, args @ ..]) => {
                let mut c = CommandBuilder::new(program);
                c.args(args);
                c
            }
            _ => {
                let mut c = CommandBuilder::new(crate::supervisor::shell());
                c.arg("-l");
                c
            }
        };
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "fubbik");
        let child = pty.slave.spawn_command(command).map_err(err)?;
        drop(pty.slave);

        let mut reader = pty.master.try_clone_reader().map_err(err)?;
        let writer = pty.master.take_writer().map_err(err)?;
        let (tx, output) = channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let mut terminal = Terminal::new(TerminalOptions {
            cols,
            rows,
            max_scrollback: SCROLLBACK,
        })
        .map_err(err)?;
        let replies = Rc::new(RefCell::new(Vec::new()));
        let title = Rc::new(RefCell::new(String::new()));
        {
            let replies = replies.clone();
            terminal
                .on_pty_write(move |_, data: &[u8]| replies.borrow_mut().extend_from_slice(data))
                .map_err(err)?;
            let title = title.clone();
            terminal
                .on_title_changed(move |t| {
                    if let Ok(text) = t.title() {
                        *title.borrow_mut() = text.to_string();
                    }
                })
                .map_err(err)?;
        }

        Ok(Self {
            terminal,
            render: RenderState::new().map_err(err)?,
            row_iter: RowIterator::new().map_err(err)?,
            cell_iter: CellIterator::new().map_err(err)?,
            encoder: key::Encoder::new().map_err(err)?,
            event: key::Event::new().map_err(err)?,
            replies,
            title,
            output,
            writer,
            master: pty.master,
            child,
            cols,
            rows,
            exited: false,
            drag_anchor: None,
            selection: (SELECTION_BG, SELECTION_FG),
        })
    }

    /// Feeds pending shell output into the terminal. True if anything changed.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.output.try_recv() {
                Ok(bytes) => {
                    self.terminal.vt_write(&bytes);
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.exited {
                        self.exited = true;
                        changed = true;
                    }
                    break;
                }
            }
        }
        let replies = std::mem::take(&mut *self.replies.borrow_mut());
        if !replies.is_empty() {
            self.write(&replies);
        }
        changed
    }

    pub fn exited(&mut self) -> bool {
        self.exited || self.child.try_wait().ok().flatten().is_some()
    }

    pub fn title(&self) -> String {
        self.title.borrow().clone()
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    pub fn resize(&mut self, cols: u16, rows: u16, cell_w: u32, cell_h: u32) {
        let (cols, rows) = (cols.max(2), rows.max(2));
        if (cols, rows) == (self.cols, self.rows) {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        let _ = self.terminal.resize(cols, rows, cell_w, cell_h);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: (cols as u32 * cell_w) as u16,
            pixel_height: (rows as u32 * cell_h) as u16,
        });
    }

    pub fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Pasted text, bracketed when the program asked for it.
    pub fn paste(&mut self, text: &str) {
        let bracketed = self.terminal.mode(Mode::BRACKETED_PASTE).unwrap_or(false);
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if bracketed {
            self.write(format!("\x1b[200~{text}\x1b[201~").as_bytes());
        } else {
            self.write(text.as_bytes());
        }
    }

    pub fn scroll(&mut self, delta_rows: isize) {
        self.terminal
            .scroll_viewport(ScrollViewport::Delta(delta_rows));
    }

    /// Encodes a key press the way the running program asked for (cursor
    /// key mode, Kitty keyboard protocol, …) and sends it to the shell.
    pub fn key(&mut self, input: &KeyInput) {
        self.terminal.scroll_viewport(ScrollViewport::Bottom);
        let mut mods = Mods::empty();
        mods.set(Mods::SHIFT, input.shift);
        mods.set(Mods::ALT, input.alt);
        mods.set(Mods::CTRL, input.ctrl);
        mods.set(Mods::SUPER, input.cmd);
        let key = map_key(&input.key);
        // Text only for printable input without ctrl; the encoder derives
        // control sequences from key + mods.
        let text = input
            .text
            .clone()
            .filter(|t| !input.ctrl && !t.chars().any(char::is_control));
        let unshifted = input
            .key
            .chars()
            .next()
            .filter(|_| input.key.chars().count() == 1);
        self.event
            .set_action(Action::Press)
            .set_key(key)
            .set_mods(mods)
            .set_consumed_mods(if text.is_some() && input.shift {
                Mods::SHIFT
            } else {
                Mods::empty()
            })
            .set_utf8(text.clone());
        if let Some(c) = unshifted {
            self.event.set_unshifted_codepoint(c);
        }
        self.encoder.set_options_from_terminal(&self.terminal);
        let mut out = Vec::new();
        if self.encoder.encode_to_vec(&self.event, &mut out).is_ok() && !out.is_empty() {
            self.write(&out);
        } else if let Some(text) = text {
            // A key Ghostty does not know (non-US layouts): send its text.
            self.write(text.as_bytes());
        }
    }

    /// Default colors, the 256-color palette and the cursor from a theme
    /// (programs can still change them with OSC sequences).
    pub fn set_colors(&mut self, theme: &crate::ghostty::Resolved) {
        let mut palette = [RgbColor::default(); 256];
        for (slot, color) in palette.iter_mut().zip(theme.palette.iter()) {
            *slot = (*color).into();
        }
        let _ = self
            .terminal
            .set_default_fg_color(Some(theme.foreground.into()));
        let _ = self
            .terminal
            .set_default_bg_color(Some(theme.background.into()));
        let _ = self
            .terminal
            .set_default_cursor_color(Some(theme.cursor.into()));
        let _ = self
            .terminal
            .set_default_color_palette(Some(VtPalette(palette)));
        self.selection = (theme.selection_background, theme.selection_foreground);
    }

    fn viewport(col: u16, row: u16) -> Point {
        Point::Viewport(PointCoordinate {
            x: col,
            y: row as u32,
        })
    }

    /// Starts a drag selection at a viewport cell (clearing any selection).
    pub fn select_start(&mut self, col: u16, row: u16) {
        let _ = self.terminal.set_selection(None);
        self.drag_anchor = self.terminal.track_grid_ref(Self::viewport(col, row)).ok();
    }

    /// Extends the drag selection to a viewport cell.
    pub fn select_extend(&mut self, col: u16, row: u16) {
        let Some(anchor) = &self.drag_anchor else {
            return;
        };
        let (Ok(Some(start)), Ok(end)) = (
            anchor.snapshot(&self.terminal),
            self.terminal.grid_ref(Self::viewport(col, row)),
        ) else {
            return;
        };
        let selection = Selection::new(start, end, false);
        let _ = self.terminal.set_selection(Some(&selection));
    }

    pub fn select_end(&mut self) {
        self.drag_anchor = None;
    }

    /// Double click: the word under the cell.
    pub fn select_word(&mut self, col: u16, row: u16) {
        let Ok(at) = self.terminal.grid_ref(Self::viewport(col, row)) else {
            return;
        };
        if let Ok(Some(selection)) = self.terminal.select_word(SelectWordOptions::new(at)) {
            let _ = self.terminal.set_selection(Some(&selection));
        }
    }

    /// Triple click: the whole (unwrapped) line under the cell.
    pub fn select_line(&mut self, col: u16, row: u16) {
        let Ok(at) = self.terminal.grid_ref(Self::viewport(col, row)) else {
            return;
        };
        if let Ok(Some(selection)) = self.terminal.select_line(SelectLineOptions::new(at)) {
            let _ = self.terminal.set_selection(Some(&selection));
        }
    }

    pub fn clear_selection(&mut self) {
        self.drag_anchor = None;
        let _ = self.terminal.set_selection(None);
    }

    /// The selection as plain text: soft-wrapped lines joined, trailing
    /// spaces trimmed. None without a selection.
    pub fn selected_text(&self) -> Option<String> {
        let options = FormatOptions::new()
            .with_emit_format(Format::Plain)
            .with_unwrap(true)
            .with_trim(true);
        let bytes = self.terminal.format_selection_alloc(None, options).ok()??;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        (!text.is_empty()).then_some(text)
    }

    /// The visible screen as styled runs.
    pub fn frame(&mut self) -> Frame {
        let Ok(snapshot) = self.render.update(&self.terminal) else {
            return Frame::default();
        };
        let colors = snapshot.colors().ok();
        let default_fg: Rgb = colors
            .as_ref()
            .map(|c| c.foreground.into())
            .unwrap_or(Rgb(0xe6, 0xe8, 0xee));
        let cursor = match (snapshot.cursor_visible(), snapshot.cursor_viewport()) {
            (Ok(true), Ok(Some(c))) => Some((c.x, c.y)),
            _ => None,
        };
        let mut frame = Frame {
            cols: snapshot.cols().unwrap_or(self.cols),
            rows: Vec::with_capacity(self.rows as usize),
            cursor,
            background: colors.as_ref().map(|c| c.background.into()),
            foreground: Some(default_fg),
            cursor_color: colors.as_ref().and_then(|c| c.cursor).map(Rgb::from),
        };
        let Ok(mut rows) = self.row_iter.update(&snapshot) else {
            return frame;
        };
        let mut text = String::new();
        while let Some(row) = rows.next() {
            let mut runs: Vec<Run> = Vec::new();
            if let Ok(mut cells) = self.cell_iter.update(row) {
                while let Some(cell) = cells.next() {
                    let wide = cell
                        .raw_cell()
                        .and_then(|c| c.wide())
                        .unwrap_or(CellWide::Narrow);
                    if matches!(wide, CellWide::SpacerTail | CellWide::SpacerHead) {
                        continue;
                    }
                    text.clear();
                    let _ = cell.graphemes_utf8(&mut text);
                    if text.is_empty() {
                        text.push(' ');
                    }
                    let selected = cell.is_selected().unwrap_or(false);
                    let style = cell.style().ok();
                    let mut fg = cell
                        .fg_color()
                        .ok()
                        .flatten()
                        .map(Rgb::from)
                        .unwrap_or(default_fg);
                    let mut bg = cell.bg_color().ok().flatten().map(Rgb::from);
                    let (bold, italic, underline, strike, inverse, invisible) = style
                        .map(|s| {
                            (
                                s.bold,
                                s.italic,
                                !matches!(s.underline, libghostty_vt::style::Underline::None),
                                s.strikethrough,
                                s.inverse,
                                s.invisible,
                            )
                        })
                        .unwrap_or_default();
                    if inverse {
                        let back = bg.or(frame.background).unwrap_or(Rgb(0x16, 0x18, 0x1d));
                        bg = Some(fg);
                        fg = back;
                    }
                    if invisible {
                        fg = bg.or(frame.background).unwrap_or(fg);
                    }
                    if selected {
                        fg = self.selection.1;
                        bg = Some(self.selection.0);
                    }
                    push_cell(&mut runs, &text, fg, bg, bold, italic, underline, strike);
                }
            }
            frame.rows.push(runs);
        }
        frame
    }
}

/// Appends a cell, extending the last run when it looks the same.
#[allow(clippy::too_many_arguments)]
pub fn push_cell(
    runs: &mut Vec<Run>,
    text: &str,
    fg: Rgb,
    bg: Option<Rgb>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
) {
    if let Some(last) = runs.last_mut()
        && last.fg == fg
        && last.bg == bg
        && last.bold == bold
        && last.italic == italic
        && last.underline == underline
        && last.strikethrough == strikethrough
    {
        last.text.push_str(text);
        return;
    }
    runs.push(Run {
        text: text.to_string(),
        fg,
        bg,
        bold,
        italic,
        underline,
        strikethrough,
    });
}

impl Drop for TermSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn text_of(frame: &Frame, row: usize) -> String {
        frame.rows[row]
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    fn wait_for(session: &mut TermSession, needle: &str) -> Frame {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            session.pump();
            let frame = session.frame();
            if (0..frame.rows.len()).any(|r| text_of(&frame, r).contains(needle))
                || Instant::now() > deadline
            {
                return frame;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn maps_gpui_key_names() {
        // Given
        let names = ["a", "Q", "enter", "left", "f5", "/", "unknown-key"];

        // When
        let keys: Vec<Key> = names.iter().map(|n| map_key(n)).collect();

        // Then
        assert_eq!(
            keys,
            [
                Key::A,
                Key::Q,
                Key::Enter,
                Key::ArrowLeft,
                Key::F5,
                Key::Slash,
                Key::Unidentified
            ]
        );
    }

    #[test]
    fn runs_merge_same_style_and_split_on_change() {
        // Given
        let mut runs = Vec::new();
        let white = Rgb(255, 255, 255);
        let red = Rgb(255, 0, 0);

        // When
        push_cell(&mut runs, "a", white, None, false, false, false, false);
        push_cell(&mut runs, "b", white, None, false, false, false, false);
        push_cell(&mut runs, "c", red, None, true, false, false, false);

        // Then
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].text, "ab");
        assert!(runs[1].bold);
    }

    #[test]
    fn shell_output_and_colors_reach_the_frame() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let mut session = TermSession::spawn(dir.path(), 60, 10).unwrap();

        // When: typed through the key encoder, like the UI does
        for c in "printf '\\033[31mred\\033[0m ok-%s\\n' marker".chars() {
            let name = if c == ' ' {
                "space".to_string()
            } else {
                c.to_string()
            };
            session.key(&KeyInput {
                key: name,
                text: Some(c.to_string()),
                ..Default::default()
            });
        }
        session.key(&KeyInput {
            key: "enter".into(),
            ..Default::default()
        });
        let frame = wait_for(&mut session, "red ok-marker");

        // Then
        let row = (0..frame.rows.len())
            .find(|&r| text_of(&frame, r).starts_with("red ok-marker"))
            .expect("output line");
        let red = frame.rows[row]
            .iter()
            .find(|run| run.text.contains("red"))
            .unwrap();
        assert!(
            red.fg.0 > 150 && red.fg.1 < 120,
            "red text is red: {:?}",
            red.fg
        );
        assert_eq!(frame.cols, 60);
        assert_eq!(frame.rows.len(), 10);
    }

    #[test]
    fn drag_word_and_line_selection_copy_text() {
        // Given: a line of known text on the screen
        let dir = tempfile::tempdir().unwrap();
        let mut session = TermSession::spawn(dir.path(), 60, 10).unwrap();
        session.write(b"clear; printf 'alpha beta gamma\\n'\r");
        let deadline = Instant::now() + Duration::from_secs(10);
        let row = loop {
            session.pump();
            let frame = session.frame();
            if let Some(r) =
                (0..frame.rows.len()).find(|&r| text_of(&frame, r) == "alpha beta gamma")
            {
                break r as u16;
            }
            assert!(Instant::now() < deadline, "output line never appeared");
            std::thread::sleep(Duration::from_millis(20));
        };

        // When
        session.select_start(6, row);
        session.select_extend(9, row);
        let dragged = session.selected_text();
        session.select_end();
        session.select_word(12, row);
        let word = session.selected_text();
        session.select_line(0, row);
        let line = session.selected_text();
        let highlighted = session.frame().rows[row as usize]
            .iter()
            .any(|r| r.bg == Some(session.selection.0));
        session.clear_selection();
        let cleared = session.selected_text();

        // Then
        assert_eq!(dragged.as_deref(), Some("beta"));
        assert_eq!(word.as_deref(), Some("gamma"));
        assert_eq!(line.as_deref(), Some("alpha beta gamma"));
        assert!(highlighted, "selected cells are drawn highlighted");
        assert_eq!(cleared, None);
    }

    /// Manual check that Claude Code's TUI draws in this terminal:
    /// `cargo test claude_tui -- --ignored --nocapture` (needs `claude` on PATH).
    #[test]
    #[ignore]
    fn claude_tui_renders() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let mut session = TermSession::spawn(dir.path(), 100, 30).unwrap();

        // When
        session.write(b"claude\r");
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            session.pump();
            std::thread::sleep(Duration::from_millis(50));
        }
        let frame = session.frame();
        session.write(b"\x03\x03");

        // Then
        let screen: Vec<String> = (0..frame.rows.len()).map(|r| text_of(&frame, r)).collect();
        println!("{}", screen.join("\n"));
        assert!(
            screen.iter().filter(|l| !l.is_empty()).count() > 3,
            "the TUI drew something"
        );
    }

    #[test]
    fn theme_colors_reach_the_frame() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let mut session = TermSession::spawn(dir.path(), 40, 6).unwrap();
        let theme = crate::ghostty::parse(
            "background = #1e1e2e\nforeground = #cdd6f4\ncursor-color = #f5e0dc\npalette = 1=#f38ba8\n",
        )
        .colors
        .resolve();

        // When
        session.set_colors(&theme);
        session.write(b"clear; printf '\\033[31mred\\033[0m\\n'\r");
        let deadline = Instant::now() + Duration::from_secs(10);
        let frame = loop {
            session.pump();
            let frame = session.frame();
            if frame.rows.iter().flatten().any(|r| r.text.trim() == "red")
                || Instant::now() > deadline
            {
                break frame;
            }
            std::thread::sleep(Duration::from_millis(20));
        };

        // Then
        assert_eq!(frame.background, Some(Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(frame.foreground, Some(Rgb(0xcd, 0xd6, 0xf4)));
        assert_eq!(frame.cursor_color, Some(Rgb(0xf5, 0xe0, 0xdc)));
        let red = frame
            .rows
            .iter()
            .flatten()
            .find(|r| r.text.trim() == "red")
            .expect("a run with just the red word");
        assert_eq!(
            red.fg,
            Rgb(0xf3, 0x8b, 0xa8),
            "ANSI red is the theme's palette 1"
        );
    }

    #[test]
    fn resize_changes_the_grid() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let mut session = TermSession::spawn(dir.path(), 40, 8).unwrap();

        // When
        session.resize(100, 30, 8, 16);
        session.pump();
        let frame = session.frame();

        // Then
        assert_eq!(session.size(), (100, 30));
        assert_eq!((frame.cols, frame.rows.len()), (100, 30));
    }
}
