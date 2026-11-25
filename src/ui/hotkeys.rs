//! Global hotkeys: keys that work while another program is in front. They
//! and their keys are Ramblio's. Only Show or hide is on until the user gives
//! the others a key, as in Ramblio.

use super::*;
use std::collections::BTreeMap;
use wx_utils::global_hotkeys::GlobalHotkeys;
use wx_utils::shortcuts::{KeyChord, ShortcutModel, TabScope, prompt_for_shortcuts};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    ShowHide,
    PlayPause,
    Next,
    Previous,
    ReadLast,
}

/// Each action, the name it's saved under, what it's called, and its key.
const ACTIONS: [(Action, &str, &str, &str); 5] = [
    (Action::ShowHide, "show_hide", "Show or hide Pubseed", "Ctrl+Alt+Shift+R"),
    (Action::PlayPause, "play_pause", "Play or pause", "Ctrl+Alt+Shift+P"),
    (Action::Next, "next", "Next in queue", "Ctrl+Alt+Shift+Right"),
    (Action::Previous, "previous", "Previous in queue", "Ctrl+Alt+Shift+Left"),
    (Action::ReadLast, "read_last", "Read the last announcement", "Ctrl+Alt+Shift+L"),
];

/// What a fresh settings file holds: every hotkey but Show or hide turned
/// off. Reset gives one its key back, which turns it on.
pub fn defaults() -> BTreeMap<String, String> {
    ["play_pause", "next", "previous", "read_last"]
        .into_iter()
        .map(|id| (id.to_string(), "none".to_string()))
        .collect()
}

/// The user's changes to the hotkeys' keys, by saved name. "none" turns one
/// off; a hotkey that isn't listed has its own key.
#[derive(Clone)]
struct Keys(BTreeMap<String, String>);

impl Keys {
    fn chord(&self, action: Action) -> Option<KeyChord> {
        let (_, id, _, key) = ACTIONS.iter().find(|entry| entry.0 == action)?;
        KeyChord::parse(self.0.get(*id).map_or(*key, String::as_str))
    }
}

impl ShortcutModel for Keys {
    type Action = Action;

    fn tabs(&self) -> Vec<String> {
        vec!["Global hotkeys".to_string()]
    }

    fn tab_scope(&self, _tab: usize) -> TabScope {
        TabScope::Global
    }

    fn actions(&self, _tab: usize) -> Vec<Action> {
        ACTIONS.iter().map(|entry| entry.0).collect()
    }

    fn action_name(&self, action: Action) -> String {
        ACTIONS
            .iter()
            .find(|entry| entry.0 == action)
            .map_or_else(String::new, |entry| entry.2.to_string())
    }

    fn chord(&self, _tab: usize, action: Action) -> Option<KeyChord> {
        Keys::chord(self, action)
    }

    fn set_chord(&mut self, _tab: usize, action: Action, chord: Option<KeyChord>) {
        if let Some(entry) = ACTIONS.iter().find(|entry| entry.0 == action) {
            let key = chord.map_or_else(|| "none".to_string(), |chord| chord.to_shortcut_string());
            self.0.insert(entry.1.to_string(), key);
        }
    }

    fn reset_action(&mut self, _tab: usize, action: Action) {
        if let Some(entry) = ACTIONS.iter().find(|entry| entry.0 == action) {
            self.0.remove(entry.1);
        }
    }

    fn reset_all(&mut self, _tab: usize) {
        self.0.clear();
    }
}

thread_local! {
    /// The registered hotkeys. Dropping them gives their keys back.
    static REGISTERED: RefCell<Option<GlobalHotkeys>> = const { RefCell::new(None) };
}

/// Registers the hotkeys that are on. With `tell`, a key another program
/// already has is spoken as well as shown in the status bar.
pub fn register(app: &App, tell: bool) {
    // The old keys have to be given back before they can be taken again.
    REGISTERED.with(|slot| slot.borrow_mut().take());
    let keys = Keys(app.cfg.borrow().hotkeys.clone());
    let bindings: Vec<(KeyChord, Action)> = ACTIONS
        .iter()
        .filter_map(|entry| keys.chord(entry.0).map(|chord| (chord, entry.0)))
        .collect();
    if bindings.is_empty() {
        return;
    }
    let (handle, taken) = GlobalHotkeys::register(bindings, |action| {
        wx_utils::post_to_ui(move || {
            if let Some(app) = super::app() {
                run(&app, action);
            }
        });
    });
    REGISTERED.with(|slot| *slot.borrow_mut() = handle);
    if let Some(chord) = taken.first() {
        let text = format!(
            "Another program already uses {}. Choose another key in File, Global hotkeys.",
            chord.to_shortcut_string()
        );
        if tell {
            app.notify(&text);
        } else {
            app.w.status.set_status_text(&text, 0);
        }
    }
}

fn run(app: &Rc<App>, action: Action) {
    match action {
        Action::ShowHide => app.show_or_hide(),
        Action::PlayPause => playback::toggle(app),
        Action::Next => playback::step_queue(app, 1),
        Action::Previous => playback::step_queue(app, -1),
        Action::ReadLast => {
            let last = app.last_said.borrow().clone();
            speech::announce(if last.is_empty() { "Nothing yet" } else { &last }, true);
        }
    }
}

/// File, Global hotkeys: changes the hotkeys' keys, and turns them on or off.
pub fn edit(app: &App) {
    let current = Keys(app.cfg.borrow().hotkeys.clone());
    let edited = app.modal(|| prompt_for_shortcuts(&app.w.frame, &current));
    if let Some(Keys(keys)) = edited {
        app.cfg.borrow_mut().hotkeys = keys;
        app.cfg.borrow().save();
        register(app, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_key_parses_and_only_show_or_hide_starts_on() {
        let fresh = Keys(defaults());
        for (action, _, _, key) in ACTIONS {
            assert!(KeyChord::parse(key).is_some(), "{key}");
            assert_eq!(fresh.chord(action).is_some(), action == Action::ShowHide);
        }
        let mut reset = fresh.clone();
        reset.reset_action(0, Action::PlayPause);
        assert_eq!(
            reset.chord(Action::PlayPause).map(|chord| chord.to_shortcut_string()),
            KeyChord::parse("Ctrl+Alt+Shift+P").map(|chord| chord.to_shortcut_string())
        );
    }
}
