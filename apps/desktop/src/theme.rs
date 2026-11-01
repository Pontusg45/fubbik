//! Colors, kept in one place so the views never hard-code a hex value.
//!
//! The palette is the built-in one or derived from a Ghostty theme (see
//! `ghostty.rs`): surfaces step from the theme's background toward its
//! foreground, text is the foreground, and accent and status colors come
//! from its ANSI slots. The terminal gets the theme itself (palette,
//! cursor, selection, font). Both can change at runtime; `generation()`
//! tells open terminals to re-apply.
//!
//! Translucent mode: the window shows the desktop through a system blur
//! (`WindowBackgroundAppearance::Blurred`), so backgrounds and surfaces are
//! tinted glass. Floating panels stay near-opaque for legibility.

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use gpui::{
    Background, BoxShadow, Hsla, WindowBackgroundAppearance, linear_color_stop, linear_gradient,
    point, px, rgb,
};

use crate::ghostty::{self, ConfigExtras, Resolved};
use crate::model::Status;
use crate::term::Rgb;

static TRANSLUCENT: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static PALETTE: RwLock<Palette> = RwLock::new(Palette::BUILTIN);
static TERMINAL: RwLock<Option<TerminalTheme>> = RwLock::new(None);

/// The UI's colors, as `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub backdrop_top: u32,
    pub backdrop_bottom: u32,
    /// The terminal's background.
    pub well: u32,
    pub sidebar: u32,
    pub surface: u32,
    pub surface_hover: u32,
    pub border: u32,
    pub input: u32,
    pub elevated: u32,
    pub text: u32,
    pub text_muted: u32,
    pub accent: u32,
    pub danger: u32,
    pub running: u32,
    pub external: u32,
    pub degraded: u32,
    pub stopped: u32,
    /// True for dark backgrounds: shadows and highlights are tuned for it.
    pub dark: bool,
    /// Distinct hues for telling things apart: repos, agents, tabs, tools.
    pub hues: [u32; 8],
}

/// What the terminal draws with.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalTheme {
    pub colors: Resolved,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub cursor_bar: bool,
}

impl Palette {
    pub const BUILTIN: Palette = Palette {
        backdrop_top: 0x1d2028,
        backdrop_bottom: 0x111317,
        well: 0x0e1014,
        sidebar: 0x1d2027,
        surface: 0x23262e,
        surface_hover: 0x2c3039,
        border: 0x30343e,
        input: 0x16181d,
        elevated: 0x1d2027,
        text: 0xe6e8ee,
        text_muted: 0x8b91a0,
        accent: 0x7c8cff,
        danger: 0xf87171,
        running: 0x4ade80,
        external: 0x60a5fa,
        degraded: 0xfbbf24,
        stopped: 0x6b7280,
        dark: true,
        hues: [
            0x7c8cff, 0x4ade80, 0xfbbf24, 0xf472b6, 0x22d3ee, 0xfb923c, 0xa78bfa, 0xa3e635,
        ],
    };

    /// The UI from a Ghostty theme.
    pub fn from_theme(theme: &Resolved) -> Palette {
        let (bg, fg) = (theme.background, theme.foreground);
        let dark = luminance(bg) < 0.5;
        let black = Rgb(0, 0, 0);
        let p = |i: usize| theme.palette[i].hex();
        Palette {
            backdrop_top: mix(bg, fg, 0.035).hex(),
            backdrop_bottom: mix(bg, black, if dark { 0.25 } else { 0.06 }).hex(),
            well: bg.hex(),
            sidebar: mix(bg, fg, 0.05).hex(),
            surface: mix(bg, fg, 0.075).hex(),
            surface_hover: mix(bg, fg, 0.13).hex(),
            border: mix(bg, fg, 0.16).hex(),
            input: mix(bg, black, if dark { 0.3 } else { 0.05 }).hex(),
            elevated: mix(bg, fg, 0.06).hex(),
            text: fg.hex(),
            text_muted: mix(fg, bg, 0.42).hex(),
            accent: p(4),
            danger: p(1),
            running: p(2),
            external: p(12),
            degraded: p(3),
            stopped: p(8),
            dark,
            // The theme's ANSI colors, so hues match it: blue, green, yellow,
            // magenta, cyan, red, bright blue, bright green.
            hues: [p(4), p(2), p(3), p(5), p(6), p(1), p(12), p(10)],
        }
    }
}

/// Linear blend: `t = 0` is `a`, `t = 1` is `b`.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let ch = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0., 255.) as u8
    };
    Rgb(ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2))
}

/// Relative luminance, 0 (black) to 1 (white).
pub fn luminance(c: Rgb) -> f32 {
    (0.2126 * c.0 as f32 + 0.7152 * c.1 as f32 + 0.0722 * c.2 as f32) / 255.
}

/// The built-in look for the terminal: the palette's well and text.
fn builtin_terminal() -> TerminalTheme {
    let b = Palette::BUILTIN;
    let rgb_of = |c: u32| Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    TerminalTheme {
        colors: Resolved {
            palette: ghostty::default_palette(),
            background: rgb_of(b.well),
            foreground: rgb_of(b.text),
            cursor: rgb_of(b.accent),
            selection_background: Rgb(0x3d, 0x47, 0x99),
            selection_foreground: Rgb(0xff, 0xff, 0xff),
        },
        font_family: None,
        font_size: None,
        cursor_bar: false,
    }
}

/// Applies a theme choice: None for the built-in look, `"follow"` for the
/// user's Ghostty config, or a Ghostty theme name. Returns a label for it.
pub fn apply(choice: Option<&str>) -> Result<String, String> {
    let (palette, terminal, label) = match choice {
        None | Some("") | Some("builtin") => {
            (Palette::BUILTIN, builtin_terminal(), "Built-in".to_string())
        }
        Some("follow") => {
            let (colors, extras) = ghostty::follow_config(
                &ghostty::read_user_config(),
                &ghostty::theme_dirs(),
                ghostty::system_is_dark(),
            );
            let resolved = colors.resolve();
            (
                Palette::from_theme(&resolved),
                terminal_theme(resolved, extras),
                "Ghostty config".to_string(),
            )
        }
        Some(name) => {
            let colors = ghostty::load_theme(name, &ghostty::theme_dirs())
                .ok_or_else(|| format!("No Ghostty theme named “{name}”"))?;
            let resolved = colors.resolve();
            (
                Palette::from_theme(&resolved),
                terminal_theme(resolved, ConfigExtras::default()),
                name.to_string(),
            )
        }
    };
    *PALETTE.write().unwrap() = palette;
    *TERMINAL.write().unwrap() = Some(terminal);
    GENERATION.fetch_add(1, Ordering::Relaxed);
    Ok(label)
}

fn terminal_theme(colors: Resolved, extras: ConfigExtras) -> TerminalTheme {
    TerminalTheme {
        colors,
        font_family: extras.font_family,
        font_size: extras.font_size,
        cursor_bar: extras.cursor_style.as_deref() == Some("bar"),
    }
}

/// Bumped on every `apply`.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

pub fn terminal() -> TerminalTheme {
    TERMINAL
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(builtin_terminal)
}

fn palette() -> Palette {
    *PALETTE.read().unwrap()
}

pub fn set_translucent(on: bool) {
    TRANSLUCENT.store(on, Ordering::Relaxed);
}

pub fn is_translucent() -> bool {
    TRANSLUCENT.load(Ordering::Relaxed)
}

pub fn window_background() -> WindowBackgroundAppearance {
    if is_translucent() {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

/// A palette color, at `glass` opacity in translucent mode.
fn tone(color: u32, glass: f32) -> Hsla {
    let c: Hsla = rgb(color).into();
    if is_translucent() {
        c.opacity(glass)
    } else {
        c
    }
}

/// The main area: a soft top-to-bottom falloff, lighter under the header,
/// so cards read as raised above it.
pub fn backdrop() -> Background {
    let p = palette();
    linear_gradient(
        180.,
        linear_color_stop(tone(p.backdrop_top, 0.58), 0.),
        linear_color_stop(tone(p.backdrop_bottom, 0.45), 1.),
    )
}

/// The terminal's background: the theme's own.
pub fn terminal_well() -> Hsla {
    tone(palette().well, 0.85)
}

/// A faint top edge that catches the light on raised surfaces.
pub fn highlight() -> Hsla {
    let base = if palette().dark {
        gpui::white()
    } else {
        gpui::black()
    };
    base.opacity(if is_translucent() { 0.07 } else { 0.045 })
}

fn shadow(alpha: f32, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color: gpui::black().opacity(if palette().dark { alpha } else { alpha * 0.45 }),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
    }
}

/// Cards and sections: a tight contact shadow plus a soft lift.
pub fn card_shadow() -> Vec<BoxShadow> {
    vec![shadow(0.28, 1., 2.), shadow(0.22, 6., 16.)]
}

/// Large framed panels, like the terminal.
pub fn frame_shadow() -> Vec<BoxShadow> {
    vec![shadow(0.35, 2., 4.), shadow(0.35, 14., 36.)]
}

pub fn sidebar() -> Hsla {
    tone(palette().sidebar, 0.35)
}

pub fn surface() -> Hsla {
    tone(palette().surface, 0.55)
}

pub fn surface_hover() -> Hsla {
    tone(palette().surface_hover, 0.6)
}

pub fn border() -> Hsla {
    let p = palette();
    if is_translucent() {
        let c: Hsla = rgb(p.text).into();
        c.opacity(0.09)
    } else {
        rgb(p.border).into()
    }
}

/// Text fields and the log view: a recessed well.
pub fn input() -> Hsla {
    if is_translucent() {
        gpui::black().opacity(0.28)
    } else {
        rgb(palette().input).into()
    }
}

/// Floating panels: the palette, the context panel, toasts.
pub fn elevated() -> Hsla {
    tone(palette().elevated, 0.94)
}

pub fn text() -> Hsla {
    rgb(palette().text).into()
}

pub fn text_muted() -> Hsla {
    let p = palette();
    if is_translucent() {
        // A little brighter on glass, where the backdrop varies.
        let lifted = mix(rgb_parts(p.text_muted), rgb_parts(p.text), 0.25);
        rgb(lifted.hex()).into()
    } else {
        rgb(p.text_muted).into()
    }
}

fn rgb_parts(c: u32) -> Rgb {
    Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn accent() -> Hsla {
    rgb(palette().accent).into()
}

/// Named hues, so each kind of thing keeps its color across the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hue {
    Blue = 0,
    Green = 1,
    Yellow = 2,
    Pink = 3,
    Cyan = 4,
    Orange = 5,
    Violet = 6,
    Lime = 7,
}

pub fn hue(h: Hue) -> Hsla {
    rgb(palette().hues[h as usize]).into()
}

/// A stable color for a name (a repo), from the theme's hues.
pub fn name_color(name: &str) -> Hsla {
    let hash = name
        .bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    rgb(palette().hues[hash as usize % 8]).into()
}

/// Each agent's color.
pub fn agent(agent: crate::sessions::Agent) -> Hsla {
    match agent {
        crate::sessions::Agent::ClaudeCode => hue(Hue::Orange),
        crate::sessions::Agent::Codex => hue(Hue::Cyan),
    }
}

/// Script tools: just, the JavaScript package managers, make.
pub fn tool(tool: &str) -> Hsla {
    match tool {
        "just" => hue(Hue::Violet),
        "make" => hue(Hue::Lime),
        _ => hue(Hue::Yellow),
    }
}

/// Log lines by what they look like: errors red, warnings yellow, success
/// green, the app's own markers muted. None: plain text.
pub fn log_line(line: &str) -> Option<Hsla> {
    let lower = line.to_ascii_lowercase();
    if line.starts_with("── ") {
        Some(text_muted())
    } else if ["error", "panicked", "failed", "fatal", "exception"]
        .iter()
        .any(|w| lower.contains(w))
    {
        Some(danger())
    } else if lower.contains("warn") {
        Some(status(Status::Degraded))
    } else if [
        "ready",
        "listening",
        "compiled successfully",
        "finished",
        " ok",
        "passed",
    ]
    .iter()
    .any(|w| lower.contains(w))
    {
        Some(status(Status::Running))
    } else {
        None
    }
}

pub fn danger() -> Hsla {
    rgb(palette().danger).into()
}

pub fn overlay() -> Hsla {
    gpui::black().opacity(if is_translucent() { 0.3 } else { 0.45 })
}

pub fn status(status: Status) -> Hsla {
    let p = palette();
    match status {
        Status::Running => rgb(p.running).into(),
        Status::External => rgb(p.external).into(),
        Status::Degraded => rgb(p.degraded).into(),
        Status::Stopped => rgb(p.stopped).into(),
        Status::Error => danger(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translucent_theme_uses_glass_backgrounds() {
        // Given
        set_translucent(false);
        let opaque = (surface().a, sidebar().a, text().a);

        // When
        set_translucent(true);
        let glass = (surface().a, sidebar().a, text().a, elevated().a);
        set_translucent(false);

        // Then
        assert_eq!(opaque, (1.0, 1.0, 1.0));
        assert!(
            glass.0 < 0.6 && glass.1 < 0.4,
            "backgrounds are see-through"
        );
        assert_eq!(glass.2, 1.0, "text stays opaque");
        assert!(glass.3 > 0.9, "floating panels stay legible");
    }

    #[test]
    fn palette_from_a_dark_theme_steps_toward_the_foreground() {
        // Given
        let theme = ghostty::parse("background = #1e1e2e\nforeground = #cdd6f4\npalette = 4=#89b4fa\npalette = 1=#f38ba8\n")
            .colors
            .resolve();

        // When
        let p = Palette::from_theme(&theme);

        // Then
        assert!(p.dark);
        assert_eq!(
            p.well, 0x1e1e2e,
            "the terminal keeps the theme's background"
        );
        assert_eq!(p.text, 0xcdd6f4);
        assert_eq!((p.accent, p.danger), (0x89b4fa, 0xf38ba8));
        let lum = |c: u32| luminance(rgb_parts(c));
        assert!(
            lum(p.well) < lum(p.surface)
                && lum(p.surface) < lum(p.surface_hover)
                && lum(p.surface_hover) < lum(p.border)
        );
    }

    #[test]
    fn light_theme_is_detected() {
        // Given
        let theme = ghostty::parse("background = #eff1f5\nforeground = #4c4f69\n")
            .colors
            .resolve();

        // When
        let p = Palette::from_theme(&theme);

        // Then
        assert!(!p.dark);
        assert_eq!(p.text, 0x4c4f69);
    }

    #[test]
    fn names_keep_their_color_and_logs_are_classified() {
        // Given
        let (a, b) = (name_color("fubbik"), name_color("fubbik"));

        // When
        let lines = [
            log_line("error[E0425]: oops"),
            log_line("warning: unused"),
            log_line("Server listening on :3000"),
            log_line("hello"),
        ];

        // Then
        assert_eq!(a, b);
        assert_eq!(lines[0], Some(danger()));
        assert_eq!(lines[1], Some(status(Status::Degraded)));
        assert_eq!(lines[2], Some(status(Status::Running)));
        assert_eq!(lines[3], None);
    }

    #[test]
    fn mix_blends_channels() {
        // Given
        let (a, b) = (Rgb(0, 0, 0), Rgb(200, 100, 50));

        // When
        let half = mix(a, b, 0.5);

        // Then
        assert_eq!(half, Rgb(100, 50, 25));
    }
}
