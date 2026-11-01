//! The Changes tab: one checkout's changed files (the repo or a worktree),
//! the diff of the one picked, staging, and committing. The fubbik chunks
//! related to what is staged show beside the commit box (`fubbik
//! check-files --staged`), so the knowledge for a change is in view before
//! it is committed.

use std::path::PathBuf;

use gpui::{AnyElement, FontWeight, SharedString};

use super::widgets::{button, disabled_button, loading, muted, pill, section_or, segment};
use super::*;
use crate::git::FileChange;
use crate::theme::Hue;

/// Diff lines drawn before "… N more lines".
const DIFF_LINES: usize = 1500;

/// A chunk `fubbik check-files` relates to the staged files.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub matched_files: Vec<String>,
}

/// The Changes tab's state: which checkout, its files, the picked file's
/// diff, and the chunks related to what is staged.
#[derive(Default)]
pub struct ChangesState {
    /// The checkout shown; None: the selected repo's own.
    pub dir: Option<PathBuf>,
    pub files: Option<(PathBuf, Loadable<Vec<FileChange>>)>,
    /// The picked file and whether its staged side is shown.
    pub pick: Option<(String, bool)>,
    pub diff: Option<Loadable<String>>,
    /// Keyed by the staged paths they were looked up for.
    pub related: Option<(Vec<String>, Loadable<Vec<RelatedChunk>>)>,
    pub committing: bool,
}

fn letter_color(letter: char) -> gpui::Hsla {
    match letter {
        'A' | '?' => theme::hue(Hue::Green),
        'D' => theme::danger(),
        'R' | 'C' => theme::hue(Hue::Violet),
        _ => theme::hue(Hue::Yellow),
    }
}

impl Workspace {
    /// The checkout the Changes tab shows.
    pub(super) fn changes_dir(&self) -> Option<PathBuf> {
        let view = self.repos.get(self.selected)?;
        Some(
            self.changes
                .dir
                .clone()
                .unwrap_or_else(|| view.entry.path.clone()),
        )
    }

    pub(super) fn load_changes(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.changes_dir() else {
            return;
        };
        // Keep the list on screen while it reloads.
        if !matches!(&self.changes.files, Some((d, Loadable::Done(_))) if *d == dir) {
            self.changes.files = Some((dir.clone(), Loadable::Loading));
        }
        let work = {
            let dir = dir.clone();
            cx.background_executor()
                .spawn(async move { crate::git::changes(&dir) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.changes_dir().as_ref() != Some(&dir) {
                    return;
                }
                // A picked file that is gone: drop it.
                if let (Ok(files), Some((path, staged))) = (&result, &this.changes.pick) {
                    let still = files.iter().any(|f| {
                        f.path == *path
                            && if *staged {
                                f.staged.is_some()
                            } else {
                                f.unstaged.is_some() || f.untracked
                            }
                    });
                    if !still {
                        this.changes.pick = None;
                        this.changes.diff = None;
                    }
                }
                this.changes.files = Some((dir, Loadable::Done(result)));
                this.load_diff(cx);
                this.load_related(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn change_files(&self) -> Option<&Vec<FileChange>> {
        match &self.changes.files {
            Some((_, Loadable::Done(Ok(files)))) => Some(files),
            _ => None,
        }
    }

    fn load_diff(&mut self, cx: &mut Context<Self>) {
        let (Some(dir), Some((path, staged))) = (self.changes_dir(), self.changes.pick.clone())
        else {
            self.changes.diff = None;
            return;
        };
        let untracked = self
            .change_files()
            .and_then(|files| files.iter().find(|f| f.path == path))
            .is_some_and(|f| f.untracked);
        if self.changes.diff.is_none() {
            self.changes.diff = Some(Loadable::Loading);
        }
        let work = {
            let path = path.clone();
            cx.background_executor()
                .spawn(async move { crate::git::diff(&dir, &path, staged, untracked) })
        };
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.changes.pick.as_ref() == Some(&(path, staged)) {
                    this.changes.diff = Some(Loadable::Done(result));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `fubbik --json check-files --staged` in the checkout, when the staged
    /// set changed since the last lookup.
    fn load_related(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.changes_dir() else {
            return;
        };
        let mut staged: Vec<String> = self
            .change_files()
            .into_iter()
            .flatten()
            .filter(|f| f.staged.is_some())
            .map(|f| f.path.clone())
            .collect();
        staged.sort();
        if staged.is_empty() {
            self.changes.related = None;
            return;
        }
        if self
            .changes
            .related
            .as_ref()
            .is_some_and(|(k, _)| *k == staged)
        {
            return;
        }
        let Some(bin) = self.global.as_ref().and_then(|g| g.fubbik_bin.clone()) else {
            self.changes.related = Some((
                staged,
                Loadable::Done(Err("the fubbik binary was not found (Setup)".into())),
            ));
            return;
        };
        self.changes.related = Some((staged.clone(), Loadable::Loading));
        let url = self.config.fubbik_url();
        let work = cx.background_executor().spawn(async move {
            let output = std::process::Command::new(bin)
                .args(["--json", "--url", &url, "check-files", "--staged"])
                .current_dir(&dir)
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }
            let text = String::from_utf8_lossy(&output.stdout);
            // Nothing related prints nothing.
            if text.trim().is_empty() {
                return Ok(Vec::new());
            }
            serde_json::from_str::<Vec<RelatedChunk>>(&text).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this
                    .changes
                    .related
                    .as_ref()
                    .is_some_and(|(k, _)| *k == staged)
                {
                    this.changes.related = Some((staged, Loadable::Done(result)));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn pick_change(&mut self, path: String, staged: bool, cx: &mut Context<Self>) {
        self.changes.pick = Some((path, staged));
        self.changes.diff = None;
        self.load_diff(cx);
    }

    /// Stages (or unstages) these paths; all of them when empty.
    pub(super) fn stage_paths(&mut self, paths: Vec<String>, stage: bool, cx: &mut Context<Self>) {
        let Some(dir) = self.changes_dir() else {
            return;
        };
        // The picked diff follows its file to the other side.
        if let Some((path, staged)) = &mut self.changes.pick
            && (paths.is_empty() || paths.contains(path))
        {
            *staged = stage;
            self.changes.diff = None;
        }
        let work = cx.background_executor().spawn(async move {
            if stage {
                crate::git::stage(&dir, &paths)
            } else {
                crate::git::unstage(&dir, &paths)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.show_error(format!("git: {error}"));
                }
                this.load_changes(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn commit_staged(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.changes_dir() else {
            return;
        };
        let message = self.commit_field.text.trim().to_string();
        if message.is_empty() {
            self.show_error("Write a commit message first");
            return;
        }
        if !self
            .change_files()
            .is_some_and(|f| f.iter().any(|f| f.staged.is_some()))
        {
            self.show_error("Nothing is staged");
            return;
        }
        self.changes.committing = true;
        let work = cx
            .background_executor()
            .spawn(async move { crate::git::commit(&dir, &message) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.changes.committing = false;
                match result {
                    Ok(summary) => {
                        this.commit_field.text.clear();
                        this.changes.related = None;
                        this.show_info(format!("Committed {summary}"));
                    }
                    // Hooks print here (a failing pre-commit check).
                    Err(error) => this.show_error(format!("Commit failed: {error}")),
                }
                this.load_changes(cx);
                this.poll_now(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_changes(
        &mut self,
        index: usize,
        snapshot: &RepoSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(dir) = self.changes_dir() else {
            return muted("No repo").into_any_element();
        };
        // Load on first view, on another checkout, and when the poll sees a
        // different number of changed files than the list has.
        let polled = match (&self.changes.dir, &snapshot.git) {
            (None, Some(Ok(git))) => Some(git.changed_files),
            _ => None,
        };
        let stale = match &self.changes.files {
            Some((d, _)) if *d != dir => true,
            None => true,
            Some((_, Loadable::Done(Ok(files)))) => polled.is_some_and(|n| n != files.len()),
            _ => false,
        };
        if stale {
            self.load_changes(cx);
        }

        // Which checkout: the repo, or one of its worktrees.
        let main = self.repos[index].entry.path.clone();
        let mut checkouts = vec![(main.clone(), "main checkout".to_string())];
        if let Some(Ok(worktrees)) = &snapshot.worktrees {
            checkouts.extend(
                worktrees
                    .iter()
                    .filter(|w| !w.main && !w.missing)
                    .map(|w| (w.path.clone(), w.name())),
            );
        }
        let chips =
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(checkouts.into_iter().enumerate().map(|(i, (path, name))| {
                    let active = path == dir;
                    let is_main = path == main;
                    segment(("checkout", i), name, active).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.changes.dir = (!is_main).then(|| path.clone());
                            this.changes.pick = None;
                            this.changes.diff = None;
                            this.load_changes(cx);
                            cx.notify();
                        },
                    ))
                }));

        let files = match &self.changes.files {
            Some((_, Loadable::Done(Ok(files)))) => files.clone(),
            Some((_, Loadable::Done(Err(error)))) => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(chips)
                    .child(muted(format!("git status failed: {error}")))
                    .into_any_element();
            }
            _ => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(chips)
                    .child(loading("Reading changes…"))
                    .into_any_element();
            }
        };
        let staged: Vec<&FileChange> = files.iter().filter(|f| f.staged.is_some()).collect();
        let unstaged: Vec<&FileChange> = files
            .iter()
            .filter(|f| f.unstaged.is_some() || f.untracked)
            .collect();

        let toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(chips))
            .child(
                button("changes-refresh", "Refresh").on_click(cx.listener(|this, _, _, cx| {
                    this.load_changes(cx);
                    cx.notify();
                })),
            )
            .child(if unstaged.is_empty() {
                disabled_button("changes-stage-all", "Stage all")
            } else {
                button("changes-stage-all", "Stage all")
                    .on_click(cx.listener(|this, _, _, cx| this.stage_paths(Vec::new(), true, cx)))
            })
            .child(if staged.is_empty() {
                disabled_button("changes-unstage-all", "Unstage all")
            } else {
                button("changes-unstage-all", "Unstage all")
                    .on_click(cx.listener(|this, _, _, cx| this.stage_paths(Vec::new(), false, cx)))
            });

        let pick = self.changes.pick.clone();
        let file_row = |file: &FileChange, on_staged: bool, cx: &mut Context<Self>| -> AnyElement {
            let letter = if on_staged {
                file.staged.unwrap_or('M')
            } else if file.untracked {
                '?'
            } else {
                file.unstaged.unwrap_or('M')
            };
            let path = file.path.clone();
            let picked = pick.as_ref() == Some(&(path.clone(), on_staged));
            // The file name first: a long folder would hide it.
            let (folder, name) = match file.path.rsplit_once('/') {
                Some((folder, name)) => (folder.to_string(), name.to_string()),
                None => (String::new(), file.path.clone()),
            };
            let name = match &file.from {
                Some(from) => format!("{} → {name}", from.rsplit('/').next().unwrap_or(from)),
                None => name,
            };
            let id = SharedString::from(format!("change-{}-{}", on_staged, file.path));
            let action_path = path.clone();
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .when(picked, |d| d.bg(theme::surface_hover()))
                .hover(|d| d.bg(theme::surface_hover()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.pick_change(path.clone(), on_staged, cx);
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(14.))
                        .flex_none()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(letter_color(letter))
                        .child(letter.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_baseline()
                        .gap_2()
                        .child(div().flex_none().text_sm().child(name))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(folder),
                        ),
                )
                .child(
                    button(
                        SharedString::from(format!("stage-{}-{}", on_staged, file.path)),
                        if on_staged { "Unstage" } else { "Stage" },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.stage_paths(vec![action_path.clone()], !on_staged, cx);
                    })),
                )
                .into_any_element()
        };
        let staged_rows: Vec<AnyElement> = staged.iter().map(|f| file_row(f, true, cx)).collect();
        let unstaged_rows: Vec<AnyElement> =
            unstaged.iter().map(|f| file_row(f, false, cx)).collect();

        // The commit box: message, button, and the knowledge for what is staged.
        let can_commit = !staged.is_empty() && !self.changes.committing;
        let commit_label = if self.changes.committing {
            "Committing…".to_string()
        } else {
            format!("Commit {} file(s)", staged.len())
        };
        let related: Vec<AnyElement> = match &self.changes.related {
            None => Vec::new(),
            Some((_, Loadable::Loading)) => {
                vec![muted("Looking up related knowledge…").into_any_element()]
            }
            Some((_, Loadable::Done(Err(error)))) => {
                vec![muted(format!("Related knowledge unavailable: {error}")).into_any_element()]
            }
            Some((_, Loadable::Done(Ok(chunks)))) if chunks.is_empty() => {
                vec![muted("No fubbik chunks cover the staged files").into_any_element()]
            }
            Some((_, Loadable::Done(Ok(chunks)))) => chunks
                .iter()
                .enumerate()
                .map(|(i, chunk)| {
                    let id = chunk.id.clone();
                    div()
                        .id(("related", i))
                        .flex()
                        .items_center()
                        .gap_2()
                        .py_0p5()
                        .cursor_pointer()
                        .hover(|d| d.text_color(theme::accent()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run(Command::OpenWeb(format!("/chunks/{id}")), cx)
                        }))
                        .child(pill(chunk.kind.clone(), theme::hue(Hue::Yellow)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .child(chunk.title.clone()),
                        )
                        .child(muted(chunk.matched_files.join(", ")))
                        .into_any_element()
                })
                .collect(),
        };
        let commit_box = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.field(
                Field::Commit,
                "Commit message — enter commits what is staged",
                window,
                cx,
            ))
            .child(div().flex().justify_end().child(if can_commit {
                button("commit", commit_label)
                    .on_click(cx.listener(|this, _, _, cx| this.commit_staged(cx)))
            } else {
                disabled_button("commit", commit_label)
            }))
            .when(!related.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text_muted())
                        .child("KNOWLEDGE FOR THE STAGED FILES"),
                )
                .children(related)
            });

        let list = div()
            .flex()
            .flex_col()
            .gap_4()
            .w(gpui::relative(0.42))
            .flex_none()
            .min_w_0()
            .child(section_or(
                format!("Staged ({})", staged.len()),
                staged_rows,
                "Nothing staged",
            ))
            .child(section_or(
                format!("Changes ({})", unstaged.len()),
                unstaged_rows,
                "Working tree clean",
            ))
            .child(commit_box);

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_start()
                    .child(list)
                    .child(self.render_diff()),
            )
            .into_any_element()
    }

    fn render_diff(&self) -> impl IntoElement {
        let body: AnyElement = match (&self.changes.pick, &self.changes.diff) {
            (None, _) => muted("Pick a file to see its diff").into_any_element(),
            (Some(_), None | Some(Loadable::Loading)) => {
                loading("Reading the diff…").into_any_element()
            }
            (Some(_), Some(Loadable::Done(Err(error)))) => {
                muted(format!("git diff failed: {error}")).into_any_element()
            }
            (Some(_), Some(Loadable::Done(Ok(text)))) if text.trim().is_empty() => {
                muted("No textual changes (a mode change or binary file)").into_any_element()
            }
            (Some(_), Some(Loadable::Done(Ok(text)))) => {
                let lines: Vec<&str> = text.lines().collect();
                let more = lines.len().saturating_sub(DIFF_LINES);
                div()
                    .flex()
                    .flex_col()
                    .font_family("Menlo")
                    .text_xs()
                    .children(lines.into_iter().take(DIFF_LINES).map(|line| {
                        let (color, bg) = if line.starts_with("+++") || line.starts_with("---") {
                            (theme::text_muted(), None)
                        } else if line.starts_with('+') {
                            (
                                theme::hue(Hue::Green),
                                Some(theme::hue(Hue::Green).opacity(0.08)),
                            )
                        } else if line.starts_with('-') {
                            (theme::danger(), Some(theme::danger().opacity(0.08)))
                        } else if line.starts_with("@@") {
                            (theme::hue(Hue::Cyan), None)
                        } else if line.starts_with("diff ") || line.starts_with("index ") {
                            (theme::text_muted(), None)
                        } else {
                            (theme::text(), None)
                        };
                        div()
                            .px_2()
                            .whitespace_nowrap()
                            .text_color(color)
                            .when_some(bg, |d, bg| d.bg(bg))
                            .child(if line.is_empty() {
                                " ".to_string()
                            } else {
                                line.replace('\t', "    ")
                            })
                    }))
                    .when(more > 0, |d| d.child(muted(format!("… {more} more lines"))))
                    .into_any_element()
            }
        };
        let title = self
            .changes
            .pick
            .as_ref()
            .map(|(path, staged)| {
                format!("{path} — {}", if *staged { "staged" } else { "not staged" })
            })
            .unwrap_or_else(|| "Diff".into());
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text_muted())
                    .truncate()
                    .child(title.to_uppercase()),
            )
            .child(
                div()
                    .id("diff")
                    .overflow_x_scroll()
                    .py_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::surface())
                    .child(body),
            )
    }
}
