//! Small building blocks shared by the views.

use gpui::{
    AnyElement, Div, ElementId, FontWeight, Hsla, IntoElement, SharedString, Stateful, div,
    prelude::*, px,
};

use crate::model::Status;
use crate::theme;

pub fn status_dot(status: Status) -> Div {
    div()
        .size_2()
        .flex_none()
        .rounded_full()
        .bg(theme::status(status))
}

pub fn status_pill(status: Status) -> Div {
    pill(status.label(), theme::status(status)).child(status_dot(status))
}

/// A colored label; the dot is optional (added by `status_pill`).
pub fn pill(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex()
        .flex_none()
        .flex_row_reverse()
        .items_center()
        .gap_1p5()
        .px_2()
        .py_0p5()
        .rounded_full()
        .max_w(px(240.))
        .overflow_hidden()
        .bg(color.opacity(0.15))
        .text_xs()
        .text_color(color)
        .child(div().truncate().child(text.into()))
}

/// A button that cannot be pressed now; `why` says so on hover-free UIs
/// by replacing nothing — callers put the reason in nearby text.
pub fn disabled_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(theme::border())
        .text_xs()
        .text_color(theme::text_muted())
        .opacity(0.6)
        .child(label.into())
}

/// One line of muted metadata, cut with an ellipsis instead of wrapping.
pub fn muted_line(text: impl Into<SharedString>) -> Div {
    div()
        .min_w(px(0.))
        .overflow_hidden()
        .text_xs()
        .text_color(theme::text_muted())
        .child(div().truncate().child(text.into()))
}

/// "Checking …" while something loads, saying what.
pub fn loading(what: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(theme::text_muted())
        .child(
            div()
                .size(px(6.))
                .rounded_full()
                .bg(theme::accent().opacity(0.7)),
        )
        .child(what.into())
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(theme::border())
        .text_xs()
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|b| b.bg(theme::surface_hover()))
        .child(label.into())
}

pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::text_muted())
        .child(text.into())
}

pub fn muted(text: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .text_color(theme::text_muted())
        .child(text.into())
}

pub fn section(title: impl Into<SharedString>, rows: Vec<AnyElement>) -> Div {
    section_or(title, rows, "Nothing here yet")
}

/// A section that says `empty` when it has no rows ("All clear", "No live
/// agents"…), instead of a generic line.
pub fn section_or(title: impl Into<SharedString>, rows: Vec<AnyElement>, empty: &str) -> Div {
    let body = if rows.is_empty() {
        vec![
            div()
                .px_4()
                .py_3()
                .child(muted(empty.to_string()))
                .into_any_element(),
        ]
    } else {
        rows
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(label(title.into().to_uppercase()))
        .child(
            div()
                .flex()
                .flex_col()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(theme::surface())
                .shadow(theme::card_shadow())
                .relative()
                .overflow_hidden()
                .child(top_highlight())
                .children(body),
        )
}

/// A 1px line along a raised surface's top edge, catching the light.
pub fn top_highlight() -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(1.))
        .bg(theme::highlight())
}

pub fn row() -> Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(theme::border())
}

/// A stat card's color, by what it counts.
fn card_color(title: &str) -> gpui::Hsla {
    use theme::Hue;
    match title {
        "Processes" => theme::hue(Hue::Green),
        "Databases" => theme::hue(Hue::Cyan),
        "Git" => theme::hue(Hue::Violet),
        "Knowledge" | "Chunks" => theme::hue(Hue::Blue),
        "Stale" => theme::hue(Hue::Yellow),
        "Orphans" => theme::hue(Hue::Pink),
        "Thin" => theme::hue(Hue::Lime),
        "Active agents" | "AI & worktrees" => theme::hue(Hue::Orange),
        _ => theme::accent(),
    }
}

pub fn stat_card(title: &'static str, value: impl Into<SharedString>) -> Div {
    let color = card_color(title);
    div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_1()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .shadow(theme::card_shadow())
        .relative()
        .overflow_hidden()
        // A colored top edge says what the card counts at a glance.
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(3.))
                .bg(color),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .child(div().size(px(6.)).rounded_full().bg(color))
                .child(muted(title)),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(value.into()),
        )
}

/// CPU history as bars, scaled to the larger of 100% and the peak.
pub fn sparkline(values: &[f32]) -> Div {
    let peak = values.iter().copied().fold(100.0_f32, f32::max);
    let height = 18.0;
    div()
        .flex()
        .flex_none()
        .items_end()
        .gap(px(1.))
        .h(px(height))
        .w(px(90.))
        .children(values.iter().rev().take(30).rev().map(|v| {
            div()
                .w(px(2.))
                .h(px((v / peak * height).max(1.)))
                .bg(theme::accent().opacity(0.8))
        }))
}

pub fn progress(done: i64, total: i64) -> Div {
    let fraction = if total > 0 {
        done as f32 / total as f32
    } else {
        0.0
    };
    div()
        .h(px(6.))
        .w(px(120.))
        .flex_none()
        .rounded_full()
        .bg(theme::border())
        .child(
            div()
                .h_full()
                .w(px(120. * fraction))
                .rounded_full()
                .bg(theme::status(Status::Running)),
        )
}

/// A text field's look; the caller attaches focus and key handling.
pub fn text_box(text: &str, placeholder: &str, focused: bool) -> Div {
    let content = if text.is_empty() && !focused {
        div()
            .text_color(theme::text_muted())
            .child(placeholder.to_string())
    } else {
        div().child(format!("{text}{}", if focused { "▏" } else { "" }))
    };
    div()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if focused {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::input())
        .text_sm()
        .cursor_text()
        .child(content)
}
