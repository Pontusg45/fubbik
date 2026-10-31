//! Ghostty themes and config. A theme is a Ghostty config fragment:
//!
//! ```text
//! palette = 4=#89b4fa
//! background = #1e1e2e
//! foreground = #cdd6f4
//! cursor-color = #f5e0dc
//! selection-background = #585b70
//! ```
//!
//! Themes are looked up the way Ghostty does: `~/.config/ghostty/themes`
//! first, then the app bundle's `Contents/Resources/ghostty/themes`. "Follow
//! Ghostty" reads `~/.config/ghostty/config`: its `theme` (also
//! `light:X,dark:Y`), then the config's own color keys on top, plus the font
//! family and cursor style for the terminal.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use libghostty_vt::style::RgbColor;
use libghostty_vt::{Terminal, TerminalOptions};

use crate::term::Rgb;

/// Colors a theme or config sets. Unset entries fall back to Ghostty's
/// defaults when resolved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeColors {
    pub palette: Vec<(u8, Rgb)>,
    pub background: Option<Rgb>,
    pub foreground: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub cursor_text: Option<Rgb>,
    pub selection_background: Option<Rgb>,
    pub selection_foreground: Option<Rgb>,
}

impl ThemeColors {
    /// Later colors win, like a config after its theme.
    pub fn overlay(&mut self, other: &ThemeColors) {
        for &(i, color) in &other.palette {
            self.palette.retain(|(j, _)| *j != i);
            self.palette.push((i, color));
        }
        macro_rules! take {
            ($($field:ident),*) => { $( if other.$field.is_some() { self.$field = other.$field; } )* };
        }
        take!(
            background,
            foreground,
            cursor,
            cursor_text,
            selection_background,
            selection_foreground
        );
    }
}

/// A theme with every color decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub palette: [Rgb; 256],
    pub background: Rgb,
    pub foreground: Rgb,
    pub cursor: Rgb,
    pub selection_background: Rgb,
    pub selection_foreground: Rgb,
}

/// Ghostty's built-in palette, read from the VT engine itself.
pub fn default_palette() -> [Rgb; 256] {
    static PALETTE: OnceLock<[Rgb; 256]> = OnceLock::new();
    *PALETTE.get_or_init(|| {
        let mut out = [Rgb(0, 0, 0); 256];
        if let Ok(terminal) = Terminal::new(TerminalOptions {
            cols: 2,
            rows: 2,
            max_scrollback: 0,
        }) && let Ok(palette) = terminal.default_color_palette()
        {
            for (slot, color) in out.iter_mut().zip(palette.0.iter()) {
                *slot = Rgb::from(*color);
            }
        }
        out
    })
}

impl ThemeColors {
    pub fn resolve(&self) -> Resolved {
        let mut palette = default_palette();
        for &(i, color) in &self.palette {
            palette[i as usize] = color;
        }
        // Ghostty's own defaults when a theme leaves these out.
        let background = self.background.unwrap_or(Rgb(0x28, 0x2c, 0x34));
        let foreground = self.foreground.unwrap_or(Rgb(0xff, 0xff, 0xff));
        Resolved {
            palette,
            background,
            foreground,
            cursor: self.cursor.unwrap_or(foreground),
            selection_background: self.selection_background.unwrap_or(foreground),
            selection_foreground: self.selection_foreground.unwrap_or(background),
        }
    }
}

/// `#rrggbb`, `rrggbb`, `#rgb`, or an X11 color name.
pub fn parse_color(value: &str) -> Option<Rgb> {
    let value = value.trim().trim_matches('"');
    let hex = value.strip_prefix('#').unwrap_or(value);
    let channel = |s: &str| u8::from_str_radix(s, 16).ok();
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(Rgb(
            channel(&hex[0..2])?,
            channel(&hex[2..4])?,
            channel(&hex[4..6])?,
        ));
    }
    if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let short = |i: usize| channel(&hex[i..=i]).map(|v| v * 17);
        return Some(Rgb(short(0)?, short(1)?, short(2)?));
    }
    RgbColor::parse_x11_color(value).ok().map(Rgb::from)
}

/// Font, cursor and keyboard settings from a Ghostty config.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigExtras {
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    /// `block`, `bar`, `underline`, `block_hollow`.
    pub cursor_style: Option<String>,
}

/// A parsed Ghostty config (or theme) file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parsed {
    pub colors: ThemeColors,
    /// The `theme` key, raw.
    pub theme: Option<String>,
    pub extras: ConfigExtras,
}

pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let colors = &mut parsed.colors;
        match key {
            "palette" => {
                if let Some((index, color)) = value.split_once('=')
                    && let (Ok(index), Some(color)) =
                        (index.trim().parse::<u8>(), parse_color(color))
                {
                    colors.palette.retain(|(i, _)| *i != index);
                    colors.palette.push((index, color));
                }
            }
            "background" => colors.background = parse_color(value),
            "foreground" => colors.foreground = parse_color(value),
            "cursor-color" => colors.cursor = parse_color(value),
            "cursor-text" => colors.cursor_text = parse_color(value),
            "selection-background" => colors.selection_background = parse_color(value),
            "selection-foreground" => colors.selection_foreground = parse_color(value),
            "theme" => parsed.theme = Some(value.trim_matches('"').to_string()),
            "font-family" if parsed.extras.font_family.is_none() => {
                parsed.extras.font_family =
                    Some(value.trim_matches('"').to_string()).filter(|f| !f.is_empty())
            }
            "font-size" => parsed.extras.font_size = value.parse().ok(),
            "cursor-style" => parsed.extras.cursor_style = Some(value.to_string()),
            _ => {}
        }
    }
    parsed
}

/// `light:Foo,dark:Bar` → the one for the appearance; a plain name → itself.
pub fn pick_theme_name(spec: &str, dark: bool) -> String {
    if !spec.contains(':') {
        return spec.to_string();
    }
    let wanted = if dark { "dark" } else { "light" };
    spec.split(',')
        .filter_map(|part| part.split_once(':'))
        .find(|(mode, _)| mode.trim() == wanted)
        .map(|(_, name)| name.trim().to_string())
        .unwrap_or_else(|| spec.to_string())
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("ghostty")
}

/// Where themes live, user's first.
pub fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![config_dir().join("themes")];
    for app in [
        PathBuf::from("/Applications/Ghostty.app"),
        home().join("Applications/Ghostty.app"),
    ] {
        dirs.push(app.join("Contents/Resources/ghostty/themes"));
    }
    dirs
}

/// Every theme name, sorted, without duplicates.
pub fn list_themes(dirs: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

/// A theme by name (or absolute path), from the first directory that has it.
pub fn load_theme(name: &str, dirs: &[PathBuf]) -> Option<ThemeColors> {
    let path = Path::new(name);
    let text = if path.is_absolute() {
        std::fs::read_to_string(path).ok()?
    } else {
        dirs.iter()
            .find_map(|dir| std::fs::read_to_string(dir.join(name)).ok())?
    };
    Some(parse(&text).colors)
}

/// The macOS appearance: dark unless the system says light.
pub fn system_is_dark() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "Dark")
        .unwrap_or(true)
}

/// What "Follow Ghostty" gives: the config's theme, its own colors on top,
/// and its font and cursor settings.
pub fn follow_config(
    config_text: &str,
    dirs: &[PathBuf],
    dark: bool,
) -> (ThemeColors, ConfigExtras) {
    let config = parse(config_text);
    let mut colors = config
        .theme
        .as_deref()
        .and_then(|spec| load_theme(&pick_theme_name(spec, dark), dirs))
        .unwrap_or_default();
    colors.overlay(&config.colors);
    (colors, config.extras)
}

pub fn read_user_config() -> String {
    std::fs::read_to_string(config_dir().join("config")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOCHA: &str = "palette = 4=#89b4fa\npalette = 1=#f38ba8\nbackground = #1e1e2e\nforeground = #cdd6f4\nselection-background = #585b70\n";

    #[test]
    fn parses_hex_short_hex_and_x11_colors() {
        // Given
        let values = ["#1e1e2e", "1e1e2e", "#fff", "red", "nonsense"];

        // When
        let colors: Vec<Option<Rgb>> = values.iter().map(|v| parse_color(v)).collect();

        // Then
        assert_eq!(colors[0], Some(Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(colors[1], colors[0]);
        assert_eq!(colors[2], Some(Rgb(255, 255, 255)));
        assert_eq!(colors[3], Some(Rgb(255, 0, 0)));
        assert_eq!(colors[4], None);
    }

    #[test]
    fn parses_a_theme_and_resolves_defaults() {
        // Given
        let parsed = parse(MOCHA);

        // When
        let resolved = parsed.colors.resolve();

        // Then
        assert_eq!(resolved.background, Rgb(0x1e, 0x1e, 0x2e));
        assert_eq!(resolved.palette[4], Rgb(0x89, 0xb4, 0xfa));
        assert_eq!(
            resolved.palette[255],
            default_palette()[255],
            "unset entries keep Ghostty's palette"
        );
        assert_eq!(
            resolved.cursor, resolved.foreground,
            "cursor defaults to the foreground"
        );
    }

    #[test]
    fn config_colors_override_its_theme_and_extras_are_read() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Mocha"), MOCHA).unwrap();
        let config = "theme = light:Latte,dark:Mocha\nbackground = #111111\nfont-family = \"PlemolJP Console NF\"\ncursor-style = bar\n";

        // When
        let (colors, extras) = follow_config(config, &[dir.path().to_path_buf()], true);

        // Then
        assert_eq!(
            colors.background,
            Some(Rgb(0x11, 0x11, 0x11)),
            "the config wins over its theme"
        );
        assert_eq!(
            colors.foreground,
            Some(Rgb(0xcd, 0xd6, 0xf4)),
            "the theme fills the rest"
        );
        assert_eq!(extras.font_family.as_deref(), Some("PlemolJP Console NF"));
        assert_eq!(extras.cursor_style.as_deref(), Some("bar"));
    }

    #[test]
    fn picks_theme_for_appearance() {
        // Given
        let spec = "light:Latte, dark:Mocha";

        // When
        let names = [
            pick_theme_name(spec, true),
            pick_theme_name(spec, false),
            pick_theme_name("Nord", true),
        ];

        // Then
        assert_eq!(names, ["Mocha", "Latte", "Nord"]);
    }

    #[test]
    fn lists_themes_user_first_without_duplicates() {
        // Given
        let user = tempfile::tempdir().unwrap();
        let bundled = tempfile::tempdir().unwrap();
        std::fs::write(user.path().join("Mine"), "").unwrap();
        std::fs::write(bundled.path().join("Mine"), "").unwrap();
        std::fs::write(bundled.path().join("Nord"), "").unwrap();

        // When
        let names = list_themes(&[user.path().to_path_buf(), bundled.path().to_path_buf()]);

        // Then
        assert_eq!(names, ["Mine", "Nord"]);
    }
}
