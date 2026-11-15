//! What the launcher just did, said for 4.2 s (the mockup's `say`): on a wide window (1280 px
//! and more) a line in the top bar between the pages and the account, right-aligned at 70 %;
//! narrower, a glass toast along the window's bottom (`hzl-float`, 20 px corners).
//!
//! Any handler can say something, even one with only `&mut App`: the line waits here and the
//! launcher's drain (every 80 ms) draws it, then lets it go.

use super::*;
use crate::glass_ui::{self as ui, white};
use horizon_theme::glass::ink;
use std::path::Path;
use std::sync::Mutex;

/// How long a line stays (`setTimeout(…, 4200)`).
const SHOWN: Duration = Duration::from_millis(4200);

/// A line's pieces: the words, and a path or command (`true`) set in mono.
type Parts = Vec<(String, bool)>;

/// The line showing: its pieces, when its 4.2 s started (moved on by the time it was held
/// under the pointer), its number (a new one restarts them), and since when it's held.
struct Line {
    parts: Parts,
    since: Instant,
    n: u64,
    held: Option<Instant>,
}

static LINE: Mutex<Option<Line>> = Mutex::new(None);

fn say_parts(parts: Parts) {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    let n = line.as_ref().map_or(1, |l| l.n + 1);
    *line = Some(Line {
        parts,
        since: Instant::now(),
        n,
        held: None,
    });
}

/// The pointer is on the toast (its time stops) or left it (its time goes on).
fn hold(on: bool) {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(l) = line.as_mut() {
        match (on, l.held) {
            (true, None) => l.held = Some(Instant::now()),
            (false, Some(from)) => {
                l.since += from.elapsed();
                l.held = None;
            }
            _ => {}
        }
    }
}

/// Says `text` now, in place of whatever was said before.
pub(super) fn say(text: impl Into<String>) {
    say_parts(vec![(text.into(), false)]);
}

/// Says `en` translated, its one `{hole}` filled with `value` in mono, wherever the language
/// puts it.
pub(super) fn say_with(en: &str, value: &str) {
    let said = tr(en);
    let parts = match (said.find('{'), said.find('}')) {
        (Some(a), Some(b)) if a < b => [
            (said[..a].to_string(), false),
            (value.to_string(), true),
            (said[b + 1..].to_string(), false),
        ]
        .into_iter()
        .filter(|(p, _)| !p.trim().is_empty())
        .collect(),
        _ => vec![(said.to_string(), false)],
    };
    say_parts(parts);
}

/// Lets the line go now: a click on the toast.
fn dismiss() {
    *LINE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The line still showing, and its number.
fn current_parts() -> Option<(Parts, u64)> {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    if line
        .as_ref()
        .is_some_and(|l| l.held.is_none() && l.since.elapsed() >= SHOWN)
    {
        *line = None;
    }
    line.as_ref().map(|l| (l.parts.clone(), l.n))
}

fn current() -> Option<(String, u64)> {
    current_parts().map(|(parts, n)| {
        let text: Vec<_> = parts.iter().map(|(p, _)| p.trim()).collect();
        (text.join(" "), n)
    })
}

/// `/Users/me/x` as `~/x`, as the mockup writes folders.
pub(super) fn tilde(path: &Path) -> String {
    let shown = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && shown.starts_with(&home) => {
            format!("~{}", &shown[home.len()..])
        }
        _ => shown,
    }
}

/// Opens `path` in the system's file manager (or its app), and says so.
pub(super) fn open(cx: &mut App, path: &Path) {
    cx.open_with_system(path);
    say_with("Opened {path}", &tilde(path));
}

impl Launcher {
    /// From the drain: whether the line changed (a new one, or the last one gone).
    pub(super) fn toast_tick(&mut self) -> bool {
        let now = current().map(|l| l.1);
        let changed = now != self.toast_seen;
        self.toast_seen = now;
        changed
    }

    /// The top bar's line, on a wide window (`hidden … xl:block`).
    pub(super) fn toast_in_topbar(&self) -> Option<Div> {
        // The Outline bar centres its pages over this spot: its toasts float at the bottom.
        (self.window_width >= 1280. && !ui::outline()).then(|| {
            ui::text(13., FontWeight::NORMAL, 0.7)
                .flex_1()
                .min_w_0()
                .pl(px(16.))
                .text_right()
                .truncate()
                .children(current().map(|l| l.0))
        })
    }

    /// The toast along the bottom, on a narrower window (`xl:hidden`).
    pub(super) fn toast_float(&self) -> Option<Div> {
        if self.window_width >= 1280. && !ui::outline() {
            return None;
        }
        if ui::outline() {
            return self.outline_toast();
        }
        let (text, n) = current()?;
        let line = ui::text(13., FontWeight::NORMAL, ink::FULL)
            .relative()
            .text_color(white(ink::FULL))
            .child(text);
        let card = crate::glass::float(20.).w_full().px(px(16.)).py(px(12.));
        Some(
            div()
                .absolute()
                .left(px(16.))
                .right(px(16.))
                .bottom(px(16.))
                .flex()
                .justify_center()
                .child(card.child(line).with_animation(
                    ElementId::named_usize("toast", n as usize),
                    crate::motion::eased(crate::motion::POP),
                    |d, p| d.opacity(p).top(px(crate::motion::POP_RISE * (1. - p))),
                )),
        )
    }

    /// The Outline look's toast: an opaque card in the bottom-right corner, edged like its
    /// plates, a tick in the accent, then the line, its path or command in mono.
    fn outline_toast(&self) -> Option<Div> {
        let (parts, n) = current_parts()?;
        let mut line = div()
            .flex()
            .flex_wrap()
            .items_baseline()
            .gap_x(px(6.))
            .min_w_0();
        for (part, mono) in parts {
            line = line.child(if mono {
                ui::mono(12., ink::SOFT).child(part.trim().to_string())
            } else {
                ui::text(13., FontWeight::MEDIUM, ink::FULL).child(part.trim().to_string())
            });
        }
        Some(
            div()
                .absolute()
                .right(px(16.))
                .bottom(px(16.))
                .left(px(16.))
                .flex()
                .justify_end()
                .child(
                    // Under the pointer it's edged brighter, takes the hover from the page under
                    // it and stays; a click lets it go.
                    div()
                        .id("toast")
                        .occlude()
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .max_w(px(560.))
                        .px(px(14.))
                        .py(px(10.))
                        .bg(ui::color(crate::glass::panel()))
                        .border_1()
                        .border_color(ui::line())
                        .hover(|s| s.border_color(white(0.4)))
                        .cursor_pointer()
                        .shadow_lg()
                        .on_hover(|on, window, _| {
                            hold(*on);
                            window.refresh();
                        })
                        .on_click(|_, window, _| {
                            dismiss();
                            window.refresh();
                        })
                        .child(ui::icon("tick-02", 16., ui::color(ui::look().accent)).flex_none())
                        .child(line)
                        .with_animation(
                            ElementId::named_usize("toast", n as usize),
                            crate::motion::eased(crate::motion::POP),
                            |d, p| d.opacity(p).top(px(crate::motion::POP_RISE * (1. - p))),
                        ),
                ),
        )
    }
}
