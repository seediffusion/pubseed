//! The app's dialogs: sign in, settings, search, and simple messages.

use super::*;
use crate::acc;
use crate::config::Appearance;

const SKIP_CHOICES: [u32; 4] = [5, 10, 15, 30];

pub fn message(app: &App, title: &str, text: &str) {
    let dialog = MessageDialog::builder(&app.w.frame, text, title)
        .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
        .build();
    app.modal(|| dialog.show_modal());
}

/// Asks before something that cannot be undone. No is the default, so
/// pressing Enter by reflex does not do it.
pub fn confirm(app: &App, title: &str, question: &str) -> bool {
    let dialog = MessageDialog::builder(&app.w.frame, question, title)
        .with_style(
            MessageDialogStyle::YesNo
                | MessageDialogStyle::NoDefault
                | MessageDialogStyle::IconQuestion,
        )
        .build();
    app.modal(|| dialog.show_modal()) == ID_YES
}

pub fn search(app: &Rc<App>) {
    let dialog = TextEntryDialog::builder(&app.w.frame, "Search for audios", "Search").build();
    if app.modal(|| dialog.show_modal()) != ID_OK {
        return;
    }
    let text = dialog.get_value().unwrap_or_default().trim().to_string();
    if text.chars().count() < 3 {
        app.notify("Type at least 3 characters to search");
    } else {
        pages::open(app, app.current_tab(), Kind::Feed(Source::Search(text)));
    }
}

/// A dialog's panel, its sizer, and helpers for labelled controls on it.
struct Form {
    dialog: Dialog,
    panel: Panel,
    sizer: BoxSizer,
}

impl Form {
    fn new(app: &App, title: &str) -> Self {
        let dialog = Dialog::builder(&app.w.frame, title).build();
        let panel = Panel::builder(&dialog).build();
        Self {
            dialog,
            panel,
            sizer: BoxSizer::builder(Orientation::Vertical).build(),
        }
    }

    fn label(&self, text: &str) {
        let label = StaticText::builder(&self.panel).with_label(text).build();
        self.sizer.add(
            &label,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
            8,
        );
    }

    fn add(&self, control: &impl WxWidget, name: &str) {
        acc::name(control, name);
        self.sizer
            .add(control, 0, SizerFlag::Expand | SizerFlag::All, 8);
    }

    fn text(&self, name: &str, style: TextCtrlStyle) -> TextCtrl {
        self.label(name);
        let text = TextCtrl::builder(&self.panel)
            .with_style(style)
            .with_size(Size::new(340, -1))
            .build();
        self.add(&text, name);
        text
    }

    fn choice(&self, name: &str, choices: Vec<String>, selected: usize) -> Choice {
        self.label(name);
        let choice = Choice::builder(&self.panel)
            .with_choices(choices)
            .with_selection(Some(selected as u32))
            .build();
        self.add(&choice, name);
        choice
    }

    /// Adds the confirm and Cancel buttons and runs the dialog. Enter presses
    /// the confirm button and Escape cancels. Returns whether it was confirmed.
    fn run(&self, app: &App, confirm: &str, focus: &impl WxWidget) -> bool {
        let row = BoxSizer::builder(Orientation::Horizontal).build();
        let ok = Button::builder(&self.panel)
            .with_label(confirm)
            .with_id(ID_OK)
            .build();
        let cancel = Button::builder(&self.panel)
            .with_label("Cancel")
            .with_id(ID_CANCEL)
            .build();
        ok.set_default();
        row.add(&ok, 0, SizerFlag::All, 8);
        row.add(&cancel, 0, SizerFlag::All, 8);
        self.sizer.add_sizer(&row, 0, SizerFlag::AlignRight, 0);
        self.panel.set_sizer(self.sizer, true);
        let outer = BoxSizer::builder(Orientation::Vertical).build();
        outer.add(&self.panel, 1, SizerFlag::Expand, 0);
        self.dialog.set_sizer_and_fit(outer, true);
        focus.set_focus();
        app.modal(|| self.dialog.show_modal()) == ID_OK
    }
}

/// Asks for an audio's new title and description. `None` if cancelled.
pub fn edit_audio(app: &App, audio: &Audio, remaining: Option<i64>) -> Option<(String, String)> {
    let form = Form::new(app, "Edit audio");
    let title = form.text("Title", TextCtrlStyle::Default);
    title.set_max_length(120);
    title.set_value(&audio.title);
    form.label("Description (optional)");
    let description = TextCtrl::builder(&form.panel)
        .with_style(TextCtrlStyle::MultiLine)
        .with_size(Size::new(340, 160))
        .build();
    description.set_max_length(5000);
    description.set_value(&audio.description);
    form.add(&description, "Description (optional)");
    if let Some(remaining) = remaining {
        form.label(&format!(
            "You can edit this {remaining} more time{}.",
            if remaining == 1 { "" } else { "s" }
        ));
    }
    let confirmed = form.run(app, "Save", &title);
    let result = (
        title.get_value().trim().to_string(),
        description.get_value().trim().to_string(),
    );
    form.dialog.destroy();
    confirmed.then_some(result)
}

pub fn sign_in(app: &Rc<App>) {
    let form = Form::new(app, "Sign in to AudioPub");
    let email = form.text("Email address", TextCtrlStyle::Default);
    let password = form.text("Password", TextCtrlStyle::Password);
    let confirmed = form.run(app, "Sign in", &email);
    let (email, password) = (email.get_value().trim().to_string(), password.get_value());
    form.dialog.destroy();
    if !confirmed || email.is_empty() {
        return;
    }
    app.notify("Signing in");
    background(
        app,
        move |api| api.login(&email, &password),
        |app, result| match result {
            Ok((token, user)) => {
                app.cfg.borrow().set_token(Some(&token));
                app.set_user(user);
                app.notify("Signed in");
                app.reload_everything();
            }
            Err(error) => message(app, "Sign in", &error),
        },
    );
}

pub fn settings(app: &Rc<App>) {
    let cfg = app.cfg.borrow().clone();
    let form = Form::new(app, "Settings");
    let names = |items: &[&str]| {
        items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
    };

    let appearance = form.choice(
        "Appearance (changes when Pubseed restarts)",
        names(&["Same as Windows", "Light", "Dark"]),
        cfg.appearance as usize,
    );
    let skip = form.choice(
        "Skip back and forward by",
        SKIP_CHOICES
            .iter()
            .map(|seconds| format!("{seconds} seconds"))
            .collect(),
        SKIP_CHOICES
            .iter()
            .position(|seconds| *seconds == cfg.skip_seconds)
            .unwrap_or(1),
    );
    let read_chat = CheckBox::builder(&form.panel)
        .with_label("Read live chat messages aloud")
        .build();
    read_chat.set_value(cfg.read_chat);
    form.sizer.add(&read_chat, 0, SizerFlag::All, 8);
    let output = form.choice(
        "Read chat with",
        names(&["Your screen reader", "An installed voice"]),
        cfg.speech_output as usize,
    );
    let installed = speech::voices();
    let mut voice_names = vec!["Windows default voice".to_string()];
    voice_names.extend(installed.iter().cloned());
    // A list filled in one call. A Choice re-measures every item each time
    // one is added, which took many seconds with hundreds of voices installed.
    form.label("Voice");
    let voice = ListBox::builder(&form.panel)
        .with_size(Size::new(340, 120))
        .build();
    acc::native_list(&voice);
    voice.set_items(&voice_names.iter().map(String::as_str).collect::<Vec<_>>());
    let current = installed.iter().position(|name| *name == cfg.voice);
    voice.set_selection(current.map_or(0, |index| index as u32 + 1), true);
    form.sizer
        .add(&voice, 0, SizerFlag::Expand | SizerFlag::All, 8);
    form.label("Voice speed");
    let rate = Slider::builder(&form.panel)
        .with_min_value(-10)
        .with_max_value(10)
        .with_value(cfg.voice_rate)
        .build();
    form.add(&rate, "Voice speed");
    let chosen_voice = move |voice: &ListBox| match voice.get_selection() {
        Some(index) if index > 0 => installed
            .get(index as usize - 1)
            .cloned()
            .unwrap_or_default(),
        _ => String::new(),
    };
    let test = Button::builder(&form.panel)
        .with_label("Test the voice")
        .build();
    form.sizer.add(&test, 0, SizerFlag::All, 8);
    {
        let chosen_voice = chosen_voice.clone();
        test.on_click(move |_| {
            speech::say(
                &chosen_voice(&voice),
                rate.value(),
                "This is how chat messages will sound.",
                true,
            );
        });
    }

    let confirmed = form.run(app, "OK", &appearance);
    if confirmed {
        let mut cfg = app.cfg.borrow_mut();
        cfg.appearance = [Appearance::System, Appearance::Light, Appearance::Dark]
            [appearance.get_selection().unwrap_or(0).min(2) as usize];
        cfg.skip_seconds = SKIP_CHOICES[skip.get_selection().unwrap_or(1).min(3) as usize];
        cfg.read_chat = read_chat.get_value();
        cfg.speech_output = if output.get_selection() == Some(1) {
            SpeechOutput::Voice
        } else {
            SpeechOutput::ScreenReader
        };
        cfg.voice = chosen_voice(&voice);
        cfg.voice_rate = rate.value();
        cfg.save();
        if let Some(menu_bar) = app.w.frame.get_menu_bar() {
            menu_bar.check_item(ID_READ_CHAT, cfg.read_chat);
            for (id, direction) in [(ID_SKIP_BACK, -1), (ID_SKIP_FORWARD, 1)] {
                if let Some(item) = menu_bar.find_item(id) {
                    item.set_label(&playback::skip_label(direction, cfg.skip_seconds));
                }
            }
        }
    }
    form.dialog.destroy();
}
