//! The daily digest in the app: Today's "Digest" button shows it in the side
//! panel; "Save to fubbik" (and, once a day after 8:00, the app on its own)
//! writes each repo's part as a note in that repo's fubbik space.

use super::*;

/// The hour after which the day's digest is saved on its own.
const SAVE_AFTER_HOUR: u32 = 8;

impl Workspace {
    pub(super) fn show_digest(&mut self) {
        let (date, _) = crate::digest::today();
        let digest = crate::digest::current(24);
        self.panel = Some(Panel {
            title: format!("Digest — {date}"),
            // The panel colors lines starting with "-" as removed (it shows
            // diffs too): bullets read as bullets here.
            body: Loadable::Done(Ok(as_bullets(&crate::digest::markdown(
                &digest,
                &format!("Digest {date}"),
            )))),
            actions: vec![("Save to fubbik".into(), Command::SaveDigest)],
        });
    }

    /// Saves each repo's part of the last day as a fubbik note in its space.
    /// `daily`: only once a day, after [`SAVE_AFTER_HOUR`], and quietly when
    /// there is nothing to save.
    pub(super) fn save_digest(&mut self, daily: bool, cx: &mut Context<Self>) {
        let (date, hour) = crate::digest::today();
        if daily {
            let saved = crate::tracker::snapshot().digest_saved_on;
            if hour < SAVE_AFTER_HOUR
                || saved.as_deref() == Some(date.as_str())
                || self.digest_saving
            {
                return;
            }
        }
        let digest = crate::digest::current(24);
        let notes: Vec<(String, String, String)> = self
            .repos
            .iter()
            .filter_map(|view| {
                let name = view.entry.name();
                let space = view
                    .knowledge
                    .as_ref()?
                    .as_ref()
                    .ok()?
                    .space
                    .as_ref()?
                    .id
                    .clone();
                let title = format!("Daily digest {date} — {name}");
                let body = crate::digest::repo_markdown(&digest, &name, &title)?;
                Some((space, title, body))
            })
            .collect();
        if daily {
            // Mark the day first: a failed save is not retried every poll.
            crate::tracker::set_digest_saved_on(&date);
        }
        if notes.is_empty() {
            if !daily {
                self.show_info(
                    "Nothing to save: no repo with a fubbik space had activity in the last day",
                );
            }
            return;
        }
        self.digest_saving = true;
        let client = self.client.clone();
        let work = cx.background_executor().spawn(async move {
            let total = notes.len();
            let failed: Vec<String> = notes
                .into_iter()
                .filter_map(|(space, title, body)| {
                    client
                        .quick_note(&space, &title, &body)
                        .err()
                        .map(|e| format!("{title}: {e}"))
                })
                .collect();
            (total, failed)
        });
        cx.spawn(async move |this, cx| {
            let (total, failed) = work.await;
            this.update(cx, |this, cx| {
                this.digest_saving = false;
                if failed.is_empty() {
                    this.show_info(format!("Daily digest saved to fubbik ({total} note(s))"));
                } else {
                    this.show_error(format!("Digest: {}", failed.join("; ")));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// `- item` → `• item`, keeping the indentation.
fn as_bullets(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let indent = line.len() - line.trim_start().len();
            match line.trim_start().strip_prefix("- ") {
                Some(rest) => format!("{}• {rest}", &line[..indent]),
                None => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
