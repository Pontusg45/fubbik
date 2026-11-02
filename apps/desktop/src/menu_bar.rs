//! The menu bar item: a short title (`fk`, or `fk 3` when three things need
//! you) and a menu of what needs you plus a few shortcuts. AppKit's
//! `NSStatusItem`, built and changed on the main thread; a click sends the
//! item's tag on a channel the UI listens to.

use std::cell::RefCell;
use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::NSString;

/// One menu line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A clickable line; its index in the list is the tag sent back.
    Item(String),
    /// Grey, not clickable.
    Label(String),
    Separator,
}

fn clicks() -> &'static (async_channel::Sender<usize>, async_channel::Receiver<usize>) {
    static CHANNEL: OnceLock<(async_channel::Sender<usize>, async_channel::Receiver<usize>)> =
        OnceLock::new();
    CHANNEL.get_or_init(async_channel::unbounded)
}

/// Clicked entries' indexes.
pub fn receiver() -> async_channel::Receiver<usize> {
    clicks().1.clone()
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and Target has no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FubbikMenuBarTarget"]
    struct Target;

    impl Target {
        #[unsafe(method(pick:))]
        fn pick(&self, sender: &NSMenuItem) {
            let _ = clicks().0.try_send(sender.tag() as usize);
        }
    }
);

struct Bar {
    item: Retained<NSStatusItem>,
    target: Retained<Target>,
    shown: (String, Vec<Entry>),
}

thread_local! {
    static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
}

/// Shows (or updates) the item. Main thread only; a no-op elsewhere and when
/// nothing changed.
pub fn set(title: &str, entries: &[Entry]) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    BAR.with(|bar| {
        let mut bar = bar.borrow_mut();
        if bar
            .as_ref()
            .is_some_and(|b| b.shown.0 == title && b.shown.1 == entries)
        {
            return;
        }
        let bar = bar.get_or_insert_with(|| {
            let item =
                NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
            let target: Retained<Target> = unsafe { msg_send![Target::alloc(mtm), init] };
            Bar {
                item,
                target,
                shown: (String::new(), Vec::new()),
            }
        });
        if let Some(button) = bar.item.button(mtm) {
            button.setTitle(&NSString::from_str(title));
        }
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        for (i, entry) in entries.iter().enumerate() {
            let line = match entry {
                Entry::Separator => NSMenuItem::separatorItem(mtm),
                Entry::Item(text) | Entry::Label(text) => {
                    let line = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(text),
                            Some(sel!(pick:)),
                            &NSString::from_str(""),
                        )
                    };
                    line.setTag(i as isize);
                    if matches!(entry, Entry::Item(_)) {
                        let target: &AnyObject = &bar.target;
                        unsafe { line.setTarget(Some(target)) };
                    } else {
                        line.setEnabled(false);
                    }
                    line
                }
            };
            menu.addItem(&line);
        }
        bar.item.setMenu(Some(&menu));
        bar.shown = (title.to_string(), entries.to_vec());
    });
}

/// Removes the item.
pub fn remove() {
    BAR.with(|bar| {
        if let Some(bar) = bar.borrow_mut().take() {
            NSStatusBar::systemStatusBar().removeStatusItem(&bar.item);
        }
    });
}
