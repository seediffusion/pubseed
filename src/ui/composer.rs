//! The New audio window. It follows Ramblio's New ramble window: its own
//! window rather than a dialog, so the rest of the app stays usable, with the
//! audio chosen first, then the title, then Publish.

use super::*;
use crate::acc;
use crate::recorder::{Recorder, Recording};
use rodio::{DeviceSinkBuilder, MixerDeviceSink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};

const MAX_BYTES: u64 = 500 * 1024 * 1024;
const SHORTEST_RECORDING: Duration = Duration::from_secs(2);
const AUDIO_FILES: &str = "Audio files (*.mp3;*.m4a;*.wav;*.ogg;*.opus;*.flac;*.aac;*.webm;*.aif;*.aiff;*.wma)|*.mp3;*.m4a;*.wav;*.ogg;*.opus;*.flac;*.aac;*.webm;*.aif;*.aiff;*.wma|All files (*.*)|*.*";

const FROM_RECORDING: u32 = 0;
const FROM_FILE: u32 = 1;

struct Upload {
    sent: Arc<AtomicU64>,
    total: u64,
}

struct Playback {
    _device: MixerDeviceSink,
    player: rodio::Player,
}

#[derive(Default)]
struct State {
    recorder: Option<Recorder>,
    recording: Option<Recording>,
    /// A chosen file and its size.
    file: Option<(PathBuf, u64)>,
    upload: Option<Upload>,
    playback: Option<Playback>,
    /// Set once published, so closing does not ask about the draft.
    published: bool,
}

pub struct Composer {
    frame: Frame,
    panel: Panel,
    from: Choice,
    record: Button,
    pause: Button,
    play_back: Button,
    discard: Button,
    time: StaticText,
    choose: Button,
    audio: TextCtrl,
    title: TextCtrl,
    description: TextCtrl,
    publish: Button,
    st: RefCell<State>,
}

fn with(action: impl FnOnce(&Rc<App>, &Rc<Composer>)) {
    if let Some(app) = super::app() {
        let composer = app.composer.borrow().clone();
        if let Some(composer) = composer {
            action(&app, &composer);
        }
    }
}

/// File, New audio: opens the window, or brings back the one already open.
pub fn open(app: &Rc<App>) {
    if app.user.borrow().is_none() {
        app.notify("Sign in to post audio. Use File, Sign in.");
        return;
    }
    let existing = app.composer.borrow().clone();
    if let Some(composer) = existing {
        composer.frame.show(true);
        composer.frame.raise();
        composer.from.set_focus();
        return;
    }

    let frame = Frame::builder()
        .with_parent(&app.w.frame)
        .with_title("New audio")
        .with_size(Size::new(520, 620))
        .build();
    let panel = Panel::builder(&frame).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = |text: &str| {
        let label = StaticText::builder(&panel).with_label(text).build();
        sizer.add(
            &label,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
            8,
        );
    };
    let named = |control: &dyn WxWidget, name: &str| acc::name(control, name);

    label("Audio from");
    let from = Choice::builder(&panel)
        .with_choices(vec![
            "A recording".to_string(),
            "A file on this computer".to_string(),
        ])
        .with_selection(Some(FROM_RECORDING))
        .build();
    named(&from, "Audio from");
    sizer.add(&from, 0, SizerFlag::All, 8);

    let row = BoxSizer::builder(Orientation::Horizontal).build();
    let button = |text: &str| {
        let button = Button::builder(&panel).with_label(text).build();
        row.add(&button, 0, SizerFlag::All, 4);
        button
    };
    let record = button("Record");
    let pause = button("Pause");
    let play_back = button("Play back");
    let discard = button("Discard");
    let choose = button("Choose file");
    sizer.add_sizer(&row, 0, SizerFlag::Left | SizerFlag::Right, 4);
    let time = StaticText::builder(&panel)
        .with_label("Recording time 0:00")
        .build();
    sizer.add(&time, 0, SizerFlag::Left | SizerFlag::Right, 8);

    label("Audio");
    let audio = TextCtrl::builder(&panel)
        .with_style(TextCtrlStyle::ReadOnly)
        .build();
    named(&audio, "Audio");
    // A single-line read-only field is left out of the Tab order otherwise.
    audio.enable_focus_from_keyboard();
    sizer.add(&audio, 0, SizerFlag::Expand | SizerFlag::All, 8);

    label("Title");
    let title = TextCtrl::builder(&panel).build();
    title.set_max_length(120);
    named(&title, "Title");
    sizer.add(&title, 0, SizerFlag::Expand | SizerFlag::All, 8);

    label("Description (optional)");
    let description = TextCtrl::builder(&panel)
        .with_style(TextCtrlStyle::MultiLine)
        .build();
    description.set_max_length(5000);
    named(&description, "Description (optional)");
    sizer.add(&description, 1, SizerFlag::Expand | SizerFlag::All, 8);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let publish = Button::builder(&panel).with_label("Publish").build();
    let cancel = Button::builder(&panel).with_label("Cancel").build();
    buttons.add(&publish, 0, SizerFlag::All, 4);
    buttons.add(&cancel, 0, SizerFlag::All, 4);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 4);
    panel.set_sizer(sizer, true);

    let composer = Rc::new(Composer {
        frame,
        panel,
        from,
        record,
        pause,
        play_back,
        discard,
        time,
        choose,
        audio,
        title,
        description,
        publish,
        st: RefCell::new(State::default()),
    });
    *app.composer.borrow_mut() = Some(composer.clone());

    from.on_selection_changed(|_| with(|_, c| c.refresh()));
    record.on_click(|_| with(toggle_recording));
    pause.on_click(|_| with(toggle_pause));
    play_back.on_click(|_| with(toggle_play_back));
    discard.on_click(|_| with(discard_recording));
    choose.on_click(|_| with(choose_file));
    publish.on_click(|_| with(publish_audio));
    cancel.on_click(|_| with(|_, c| c.frame.close(false)));
    frame.on_char_hook(|event| {
        let escape = matches!(&event, WindowEventData::Keyboard(key) if key.get_key_code() == Some(WXK_ESCAPE));
        if escape {
            with(|_, c| c.frame.close(false));
        }
        event.skip(!escape);
    });
    frame.on_close(|event| {
        let mut allow = true;
        with(|app, c| {
            allow = c.may_close(app);
            if allow {
                c.stop_everything();
                *app.composer.borrow_mut() = None;
            }
        });
        if !allow && let WindowEventData::General(e) = &event {
            e.veto();
        }
        event.skip(allow);
    });

    composer.refresh();
    composer.frame.show(true);
    composer.frame.centre();
    composer.from.set_focus();
}

impl Composer {
    fn uses_file(&self) -> bool {
        self.from.get_selection() == Some(FROM_FILE)
    }

    /// The file that would be published, if there is one.
    fn chosen_audio(&self) -> Option<PathBuf> {
        let st = self.st.borrow();
        if self.uses_file() {
            st.file.as_ref().map(|(path, _)| path.clone())
        } else {
            st.recording
                .as_ref()
                .map(|recording| recording.file.path().to_path_buf())
        }
    }

    fn has_draft(&self) -> bool {
        let st = self.st.borrow();
        st.recorder.is_some()
            || st.recording.is_some()
            || st.file.is_some()
            || !self.title.get_value().trim().is_empty()
            || !self.description.get_value().trim().is_empty()
    }

    /// Brings the controls and the Audio line up to date.
    fn refresh(&self) {
        let file = self.uses_file();
        let st = self.st.borrow();
        let recording = st.recorder.is_some();
        for (button, shown) in [
            (&self.record, !file),
            (&self.pause, !file),
            (&self.play_back, !file),
            (&self.discard, !file),
            (&self.choose, file),
        ] {
            button.show(shown);
        }
        self.time.show(!file);
        self.from.enable(!recording && st.upload.is_none());
        self.record
            .set_label(match (recording, st.recording.is_some()) {
                (true, _) => "Stop recording",
                (false, true) => "Record again",
                (false, false) => "Record",
            });
        self.pause.enable(recording);
        self.pause
            .set_label(if st.recorder.as_ref().is_some_and(Recorder::is_paused) {
                "Resume"
            } else {
                "Pause"
            });
        self.play_back.enable(!recording && st.recording.is_some());
        self.play_back.set_label(if st.playback.is_some() {
            "Stop playback"
        } else {
            "Play back"
        });
        self.discard.enable(!recording && st.recording.is_some());
        self.publish.enable(st.upload.is_none());
        drop(st);
        self.show_status();
        self.panel.layout();
    }

    fn show_status(&self) {
        let st = self.st.borrow();
        let text = if let Some(upload) = &st.upload {
            // In tens, so a screen reader is not told every percent.
            let percent = upload.sent.load(Ordering::Relaxed) * 100 / upload.total.max(1);
            format!("Uploading {}%", percent.min(100) / 10 * 10)
        } else if self.uses_file() {
            match &st.file {
                Some((path, size)) => format!(
                    "{}, {:.1} MB, ready to publish",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("the file"),
                    *size as f64 / 1_048_576.0
                ),
                None => "No file chosen".to_string(),
            }
        } else if st.recorder.is_some() {
            "Recording".to_string()
        } else {
            match &st.recording {
                Some(recording) => format!(
                    "Recording, {}, ready to publish",
                    playback::clock(recording.length)
                ),
                None => "No audio yet".to_string(),
            }
        };
        if self.audio.get_value() != text {
            self.audio.set_value(&text);
        }
    }

    fn stop_playback(&self) {
        self.st.borrow_mut().playback = None;
    }

    /// Stops recording and playing, as the window closes.
    fn stop_everything(&self) {
        self.stop_playback();
        if let Some(recorder) = self.st.borrow_mut().recorder.take() {
            let _ = recorder.finish();
        }
    }

    /// Asks before a draft is thrown away. False keeps the window open.
    fn may_close(&self, app: &App) -> bool {
        if self.st.borrow().upload.is_some() {
            app.notify("Still uploading. Wait for it to finish.");
            return false;
        }
        if self.st.borrow().published || !self.has_draft() {
            return true;
        }
        let dialog = MessageDialog::builder(
            &self.frame,
            "Discard this audio? The audio and what you typed will be lost.",
            "Discard audio",
        )
        .with_style(
            MessageDialogStyle::YesNo
                | MessageDialogStyle::NoDefault
                | MessageDialogStyle::IconQuestion,
        )
        .build();
        dialog.set_yes_no_labels("Discard audio", "Keep editing");
        // Windows would otherwise put focus back on the window's first control.
        let focused = unsafe { GetFocus() }.0 as isize;
        let discard = dialog.show_modal() == ID_YES;
        if !discard {
            wxdragon::call_after(Box::new(move || unsafe {
                let _ = SetFocus(Some(HWND(focused as *mut _)));
            }));
        }
        discard
    }

    fn ask(&self, title: &str, question: &str, yes: &str) -> bool {
        let dialog = MessageDialog::builder(&self.frame, question, title)
            .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion)
            .build();
        dialog.set_yes_no_labels(yes, "Cancel");
        dialog.show_modal() == ID_YES
    }

    fn tell(&self, title: &str, text: &str) {
        let dialog = MessageDialog::builder(&self.frame, text, title)
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build();
        dialog.show_modal();
    }
}

fn toggle_recording(app: &Rc<App>, c: &Rc<Composer>) {
    let recorder = c.st.borrow_mut().recorder.take();
    if let Some(recorder) = recorder {
        match recorder.finish() {
            Err(error) => app.notify(&error),
            Ok(recording) if recording.length < SHORTEST_RECORDING => {
                app.notify("That recording is too short. Record at least 2 seconds.");
            }
            Ok(recording) => {
                app.notify(&format!("Stopped, {}", playback::clock(recording.length)));
                c.st.borrow_mut().recording = Some(recording);
            }
        }
        c.refresh();
        return;
    }
    if c.st.borrow().recording.is_some()
        && !c.ask(
            "Record again",
            "Record again? This replaces the recording you have.",
            "Record again",
        )
    {
        return;
    }
    c.stop_playback();
    match Recorder::start() {
        Ok(recorder) => {
            let mut st = c.st.borrow_mut();
            st.recording = None;
            st.recorder = Some(recorder);
            drop(st);
            app.notify("Recording");
        }
        Err(error) => c.tell("Recording", &error),
    }
    c.refresh();
}

fn toggle_pause(app: &Rc<App>, c: &Rc<Composer>) {
    if let Some(recorder) = c.st.borrow().recorder.as_ref() {
        let pause = !recorder.is_paused();
        recorder.set_paused(pause);
        app.notify(if pause { "Paused" } else { "Recording" });
    }
    c.refresh();
}

fn toggle_play_back(app: &Rc<App>, c: &Rc<Composer>) {
    if c.st.borrow().playback.is_some() {
        c.stop_playback();
        c.refresh();
        return;
    }
    let path = match c.st.borrow().recording.as_ref() {
        Some(recording) => recording.file.path().to_path_buf(),
        None => return,
    };
    let played = std::fs::File::open(&path)
        .map_err(|e| e.to_string())
        .and_then(|file| rodio::Decoder::try_from(file).map_err(|e| e.to_string()))
        .and_then(|decoder| {
            let device = DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
            let player = rodio::Player::connect_new(device.mixer());
            player.set_volume(app.cfg.borrow().volume.min(100) as f32 / 100.0);
            player.append(decoder);
            Ok(Playback {
                _device: device,
                player,
            })
        });
    match played {
        Ok(playback) => c.st.borrow_mut().playback = Some(playback),
        Err(error) => app.notify(&format!("Couldn't play the recording: {error}")),
    }
    c.refresh();
}

fn discard_recording(app: &Rc<App>, c: &Rc<Composer>) {
    c.stop_playback();
    c.st.borrow_mut().recording = None;
    app.notify("Recording discarded");
    c.refresh();
    c.record.set_focus();
}

fn choose_file(app: &Rc<App>, c: &Rc<Composer>) {
    let dialog = FileDialog::builder(&c.frame)
        .with_message("Choose an audio file")
        .with_wildcard(AUDIO_FILES)
        .with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist)
        .build();
    if dialog.show_modal() != ID_OK {
        return;
    }
    let Some(path) = dialog.get_path().map(PathBuf::from) else {
        return;
    };
    let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    if size > MAX_BYTES {
        c.tell(
            "Choose file",
            "That file is over 500 MB, which is the most AudioPub takes.",
        );
        return;
    }
    if size == 0 {
        c.tell("Choose file", "That file is empty.");
        return;
    }
    c.st.borrow_mut().file = Some((path, size));
    c.refresh();
    app.notify("Ready to publish");
}

fn publish_audio(app: &Rc<App>, c: &Rc<Composer>) {
    let title = c.title.get_value().trim().to_string();
    let description = c.description.get_value().trim().to_string();
    let problem = if c.st.borrow().recorder.is_some() {
        Some(("Stop recording first", &c.record as &dyn WxWidget))
    } else if c.chosen_audio().is_none() {
        let control: &dyn WxWidget = if c.uses_file() { &c.choose } else { &c.record };
        Some(("Add audio first", control))
    } else if title.is_empty() {
        Some(("Add a title", &c.title as &dyn WxWidget))
    } else if title.chars().count() < 3 {
        Some((
            "The title needs at least 3 characters",
            &c.title as &dyn WxWidget,
        ))
    } else {
        None
    };
    if let Some((problem, control)) = problem {
        app.notify(problem);
        control.set_focus();
        return;
    }
    let Some(path) = c.chosen_audio() else { return };
    let total = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    if total > MAX_BYTES {
        c.tell(
            "Publish",
            "This audio is over 500 MB, which is the most AudioPub takes.",
        );
        return;
    }
    c.stop_playback();
    let sent = Arc::new(AtomicU64::new(0));
    c.st.borrow_mut().upload = Some(Upload {
        sent: sent.clone(),
        total,
    });
    c.refresh();
    app.notify("Uploading");
    background(
        app,
        move |api| api.upload(&path, &title, &description, sent),
        |app, result| {
            let Some(c) = app.composer.borrow().clone() else {
                return;
            };
            c.st.borrow_mut().upload = None;
            match result {
                Ok(audio_id) => {
                    c.st.borrow_mut().published = true;
                    c.frame.close(true);
                    app.w.frame.raise();
                    // The list under the new audio's page then has it at the top.
                    pages::refresh_feeds(app);
                    pages::open(app, app.current_tab(), Kind::Audio(audio_id));
                    app.notify("Published");
                }
                Err(error) => {
                    c.refresh();
                    c.tell("Publish", &format!("Couldn't publish: {error}"));
                }
            }
        },
    );
}

/// Runs with the main window's timer: keeps the recording time and the upload
/// progress current, and notices when playback ends.
pub fn tick(app: &App) {
    let Some(c) = app.composer.borrow().clone() else {
        return;
    };
    let (length, playback_ended, uploading) = {
        let st = c.st.borrow();
        (
            st.recorder.as_ref().map(Recorder::length),
            st.playback
                .as_ref()
                .is_some_and(|playback| playback.player.empty()),
            st.upload.is_some(),
        )
    };
    if let Some(length) = length {
        c.time
            .set_label(&format!("Recording time {}", playback::clock(length)));
    }
    if playback_ended {
        c.stop_playback();
        c.refresh();
    }
    if uploading {
        c.show_status();
    }
}
