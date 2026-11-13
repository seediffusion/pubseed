//! Screen reader plumbing for wxWidgets controls on Windows. The techniques
//! here follow Pubsplash (MIT, Brandon Cross), which documents why each is
//! needed.

use wxdragon::accessible::{AccStatus, Accessible, AccessibleImpl};
use wxdragon::prelude::*;

struct NameOnly(String);

impl AccessibleImpl for NameOnly {
    // The trait's default answers "zero children" instead of deferring.
    fn get_child_count(&self) -> (AccStatus, i32) {
        (AccStatus::NotImplemented, 0)
    }

    fn get_name(&self, child_id: i32) -> (AccStatus, Option<String>) {
        if child_id == 0 {
            (AccStatus::Ok, Some(self.0.clone()))
        } else {
            (AccStatus::NotImplemented, None)
        }
    }
}

/// Gives a control the name a screen reader announces. wx does not pass on
/// the `StaticText` label beside a text field, slider or choice. Not for
/// lists, which take their name from their label: use [`native_list`].
pub fn name(widget: &dyn WxWidget, name: &str) {
    widget.set_accessible(Accessible::new(widget, NameOnly(name.to_string())));
}

struct ListAccessible;

impl AccessibleImpl for ListAccessible {
    // The trait's default answers "zero children" instead of deferring.
    fn get_child_count(&self) -> (AccStatus, i32) {
        (AccStatus::NotImplemented, 0)
    }

    // "No row of mine has focus", whatever the truth.
    fn get_focus(&self) -> (AccStatus, i32, Option<Accessible>) {
        (AccStatus::Ok, 0, None)
    }
}

/// Stops a screen reader reading a list's row twice when the list gains
/// focus.
///
/// Windows announces focus on the list, then the list announces focus on its
/// row once wx has finished its own focus handling. NVDA answers the first
/// event by asking the list which row has focus and reading that row, and it
/// never treats a second focus event on a row as a duplicate, so whenever it
/// sees the two events apart it reads the row twice. Declining to name a
/// focused row leaves the first event as "Audios list" and the row to the
/// second. Everything else about the list is still described by Windows, and
/// its name comes from the `StaticText` created just before it.
pub fn native_list(list: &ListBox) {
    list.set_accessible(Accessible::new(list, ListAccessible));
}
