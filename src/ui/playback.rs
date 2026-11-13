//! What the Player menu, the player bar and the playback keys do.

use super::*;
use crate::player;
use std::time::Duration;

pub fn clock(time: Duration) -> String {
    let seconds = time.as_secs();
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn start(app: &Rc<App>, now: Now, urls: Vec<String>) {
    let live = matches!(now, Now::Live(_));
    let audio_id = match &now {
        Now::Audio(audio) => Some(audio.id.clone()),
        Now::Live(_) => None,
    };
    let ticket = app.player.borrow_mut().begin();
    *app.now.borrow_mut() = Some(now);
    background(
        app,
        move |api| player::open(api, &urls, live),
        move |app, opened| {
            let current = app.player.borrow().is_current(ticket);
            let outcome = match opened {
                Ok(decoded) => app.player.borrow_mut().start(ticket, decoded),
                Err(error) => Err(error),
            };
            match outcome {
                Ok(true) => {
                    if let Some(audio_id) = audio_id {
                        background(app, move |api| api.register_play(&audio_id), |_, _| {});
                    }
                }
                Err(error) if current => {
                    *app.now.borrow_mut() = None;
                    app.notify(&error);
                }
                _ => {}
            }
        },
    );
}

fn play_audio(app: &Rc<App>, audio: Audio) {
    // The upload as it was sent, then the site's AAC copy of it for formats
    // this player cannot decode.
    let urls = vec![
        format!("{}/audio/{}", app.api.base, audio.id),
        format!("{}/audio/{}.aac", app.api.base, audio.id),
    ];
    start(app, Now::Audio(audio), urls);
}

pub fn play_stream(app: &Rc<App>, stream: Stream) {
    let broadcaster = stream
        .user
        .as_ref()
        .map(|user| user.id.clone())
        .unwrap_or_default();
    let urls = vec![format!("{}/{broadcaster}", app.api.stream_base)];
    start(app, Now::Live(stream), urls);
}

/// "Play all from here": plays these one after another.
pub fn play_queue(app: &Rc<App>, audios: Vec<Audio>) {
    if let Some(first) = audios.first().cloned() {
        *app.queue.borrow_mut() = (audios, 0);
        play_audio(app, first);
    }
}

/// Space on an audio: plays it, or pauses and resumes it if it is the one playing.
pub fn play_or_toggle(app: &Rc<App>, audio: Audio) {
    let is_current =
        matches!(&*app.now.borrow(), Some(Now::Audio(current)) if current.id == audio.id);
    if is_current {
        toggle(app);
    } else {
        play_queue(app, vec![audio]);
    }
}

pub fn toggle(app: &Rc<App>) {
    let player = app.player.borrow();
    let loading = !player.is_loaded() && app.now.borrow().is_some();
    let live = matches!(&*app.now.borrow(), Some(Now::Live(_)));
    if loading {
        app.notify("Still loading");
    } else if live || (!player.is_loaded() && app.room.borrow().is_some()) {
        // In a stream's room this stops its audio, or starts it again.
        drop(player);
        live::toggle_audio(app);
    } else if !player.is_loaded() {
        app.notify("Nothing playing. Press Space on an audio to play it.");
    } else {
        let pause = !player.is_paused();
        player.set_paused(pause);
        if pause {
            app.notify("Paused");
        }
    }
}

pub fn stop(app: &App) {
    app.player.borrow_mut().stop();
    *app.now.borrow_mut() = None;
}

pub fn skip(app: &App, direction: i64) {
    let player = app.player.borrow();
    if matches!(&*app.now.borrow(), Some(Now::Live(_))) {
        app.notify("A live stream can't be skipped through");
    } else if player.is_loaded() {
        let step = app.cfg.borrow().skip_seconds as i64 * direction;
        player.seek(Duration::from_secs(
            (player.position().as_secs() as i64 + step).max(0) as u64,
        ));
    }
}

pub fn step_queue(app: &Rc<App>, direction: i64) {
    let next = {
        let mut queue = app.queue.borrow_mut();
        let target = queue.1 as i64 + direction;
        match queue.0.get(target.max(0) as usize).cloned() {
            Some(audio) if target >= 0 => {
                queue.1 = target as usize;
                Some(audio)
            }
            _ => None,
        }
    };
    match next {
        Some(audio) => play_audio(app, audio),
        None => app.notify(if direction > 0 {
            "End of the queue"
        } else {
            "Start of the queue"
        }),
    }
}

pub fn change_volume(app: &App, change: i32) {
    let volume = (app.cfg.borrow().volume as i32 + change).clamp(0, 100) as u32;
    app.cfg.borrow_mut().volume = volume;
    app.player.borrow().set_volume(volume);
    app.w.volume.set_value(volume as i32);
    app.notify(&format!("Volume {volume}"));
}

pub fn whats_playing(app: &App) {
    let player = app.player.borrow();
    let text = match &*app.now.borrow() {
        None => "Nothing playing".to_string(),
        Some(Now::Live(stream)) => format!("Live: {}", stream.title),
        Some(Now::Audio(audio)) => {
            let by = audio
                .user
                .as_ref()
                .map_or("someone", |user| user.display_name.as_str());
            let length = player
                .duration
                .map(|duration| format!(" of {}", clock(duration)))
                .unwrap_or_default();
            let state = if !player.is_loaded() {
                ", loading"
            } else if player.is_paused() {
                ", paused"
            } else {
                ""
            };
            format!(
                "{}, by {by}, {}{length}{state}",
                audio.title,
                clock(player.position())
            )
        }
    };
    app.notify(&text);
}

/// Moves to a point in the audio, given as a fraction of its length.
pub fn seek_fraction(app: &App, fraction: f64) {
    let player = app.player.borrow();
    if let Some(duration) = player.duration {
        player.seek(duration.mul_f64(fraction.clamp(0.0, 1.0)));
    }
}

/// A key on the Position slider. The native slider would move from wherever
/// it was last drawn, and backwards for Up and Page Up.
pub fn position_key(app: &Rc<App>, code: i32) -> bool {
    let (position, duration) = {
        let player = app.player.borrow();
        (
            player.position().as_secs_f64(),
            player.duration.map(|duration| duration.as_secs_f64()),
        )
    };
    let target = match code {
        WXK_LEFT | WXK_DOWN => position - 5.0,
        WXK_RIGHT | WXK_UP => position + 5.0,
        WXK_PAGEDOWN => position - 30.0,
        WXK_PAGEUP => position + 30.0,
        WXK_HOME => 0.0,
        WXK_END => duration.unwrap_or(position),
        WXK_SPACE => {
            toggle(app);
            return true;
        }
        // The number keys: 0 is the start, 5 is halfway.
        48..=57 => duration.unwrap_or(0.0) * f64::from(code - 48) / 10.0,
        _ => return false,
    };
    if let Some(duration) = duration.filter(|duration| *duration > 0.0) {
        let target = target.clamp(0.0, duration);
        app.player.borrow().seek(Duration::from_secs_f64(target));
        app.w
            .position
            .set_value((target / duration * 1000.0) as i32);
    }
    true
}

/// Runs four times a second: keeps the player bar current and moves on when
/// an audio ends.
pub fn tick(app: &Rc<App>) {
    let (finished, playing, fraction) = {
        let player = app.player.borrow();
        let fraction = player
            .duration
            .filter(|duration| !duration.is_zero())
            .map_or(0.0, |duration| {
                player.position().as_secs_f64() / duration.as_secs_f64()
            });
        (
            player.finished(),
            player.is_loaded() && !player.is_paused(),
            fraction,
        )
    };
    if finished {
        let was_live = matches!(&*app.now.borrow(), Some(Now::Live(_)));
        let has_next = {
            let queue = app.queue.borrow();
            !was_live && queue.1 + 1 < queue.0.len()
        };
        if has_next {
            step_queue(app, 1);
        } else {
            stop(app);
            if was_live && app.room.borrow().is_some() {
                app.notify("The stream's audio stopped. Its page has a button to play it again.");
            }
        }
        return;
    }
    let label = if playing { "Pause" } else { "Play" };
    if app.w.play_button.get_label() != label {
        app.w.play_button.set_label(label);
    }
    // Moving a focused slider makes a screen reader read every change.
    let value = (fraction * 1000.0) as i32;
    if !app.w.position.has_focus() && app.w.position.value() != value {
        app.w.position.set_value(value);
    }
}
