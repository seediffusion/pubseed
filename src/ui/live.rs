//! Joining a live stream: its room, its audio and its chat.

use super::*;
use crate::api::{LiveEvent, StreamPage};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Room {
    pub stream: Stream,
    pub page_id: u64,
    /// Stops the thread following the stream's events.
    pub cancel: Arc<AtomicBool>,
    listeners: i64,
}

pub fn join(app: &Rc<App>, stream_id: &str) {
    let already_in = app
        .room
        .borrow()
        .as_ref()
        .is_some_and(|room| room.stream.id == stream_id);
    if already_in {
        app.select_tab(TAB_LIVE);
        return;
    }
    app.notify("Joining");
    let stream_id = stream_id.to_string();
    background(
        app,
        move |api| api.stream(&stream_id),
        |app, result| match result {
            Ok(joined) => enter(app, joined),
            Err(error) => app.notify(&error),
        },
    );
}

fn enter(app: &Rc<App>, joined: StreamPage) {
    if app.room.borrow().is_some() {
        leave(app, "");
    }
    let stream = joined.stream;
    let page = pages::open(app, TAB_LIVE, Kind::Room(stream.id.clone()));
    pages::show_room(&page, &stream, joined.chats);
    let cancel = Arc::new(AtomicBool::new(false));
    *app.room.borrow_mut() = Some(Room {
        stream: stream.clone(),
        page_id: page.id,
        cancel: cancel.clone(),
        listeners: stream.active_listeners,
    });

    let (api, stream_id, page_id) = (app.api.clone(), stream.id.clone(), page.id);
    std::thread::spawn(move || {
        api.follow_events(&stream_id, &cancel, |event| {
            wxdragon::call_after(Box::new(move || {
                if let Some(app) = super::app() {
                    on_event(&app, page_id, event);
                }
            }));
            wxdragon::app::wake_up_idle();
        });
    });

    app.w
        .status
        .set_status_text(&format!("Live: {}", stream.title), 2);
    app.select_tab(TAB_LIVE);
    let host = stream
        .user
        .as_ref()
        .map_or("someone", |user| user.display_name.as_str());
    app.notify(&format!("Joined {}, by {host}", stream.title));
    playback::play_stream(app, stream);
}

fn on_event(app: &Rc<App>, page_id: u64, event: LiveEvent) {
    let in_this_room = app
        .room
        .borrow()
        .as_ref()
        .is_some_and(|room| room.page_id == page_id);
    let Some(page) = app.find_page(page_id).filter(|_| in_this_room) else {
        return;
    };
    match event {
        LiveEvent::Chat(chat) => {
            let mine = app
                .user
                .borrow()
                .as_ref()
                .is_some_and(|user| user.id == chat.user.id);
            let text = format!("{}: {}", chat.user.display_name, chat.content);
            pages::append_chat(&page, chat);
            if app.cfg.borrow().read_chat && !mine {
                app.speak(&text);
            }
        }
        LiveEvent::ChatDeleted(chat_id) => pages::remove_item(&page, &chat_id),
        LiveEvent::Listeners(count) => {
            if let Some(room) = app.room.borrow_mut().as_mut() {
                room.listeners = count;
            }
        }
        LiveEvent::Finished => leave(app, "The stream has ended"),
    }
}

/// Leaves the stream: closes its room and stops its audio and chat.
pub fn leave(app: &Rc<App>, message: &str) {
    let Some(room) = app.room.borrow_mut().take() else {
        app.notify("You're not in a stream");
        return;
    };
    room.cancel.store(true, Ordering::Relaxed);
    if matches!(&*app.now.borrow(), Some(Now::Live(_))) {
        playback::stop(app);
    }
    app.remove_page(room.page_id);
    app.w.status.set_status_text("", 2);
    if !message.is_empty() {
        app.notify(message);
    }
}

/// Stops the stream's audio while staying in its chat, or plays it again.
pub fn toggle_audio(app: &Rc<App>) {
    let Some(stream) = app.room.borrow().as_ref().map(|room| room.stream.clone()) else {
        return;
    };
    if matches!(&*app.now.borrow(), Some(Now::Live(_))) {
        playback::stop(app);
        app.notify("Stream audio stopped");
    } else {
        playback::play_stream(app, stream);
    }
}

pub fn listeners(app: &App) {
    match app.room.borrow().as_ref() {
        Some(room) => app.notify(&format!("{} listening", room.listeners)),
        None => app.notify("You're not in a stream"),
    }
}
