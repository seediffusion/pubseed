//! The pages a tab can show: lists of audios, the live streams, the
//! notifications, an audio's page with its comments, and a stream's room.

use super::*;
use crate::acc;
use crate::api::{Chat, Comment, Notification, Profile, Result};

pub enum Kind {
    Feed(Source),
    Live,
    Notifications,
    Audio(String),
    Room(String),
}

#[derive(Clone)]
pub enum Item {
    Audio(Audio),
    Stream(Stream),
    /// A comment and how deeply it is nested in its thread.
    Comment(Comment, usize),
    Chat(Chat),
    Notification(Notification),
}

fn name_of(user: &Option<User>) -> &str {
    user.as_ref()
        .map_or("someone", |user| user.display_name.as_str())
}

impl Item {
    fn id(&self) -> &str {
        match self {
            Item::Audio(audio) => &audio.id,
            Item::Stream(stream) => &stream.id,
            Item::Comment(comment, _) => &comment.id,
            Item::Chat(chat) => &chat.id,
            Item::Notification(notification) => &notification.id,
        }
    }

    fn author_id(&self) -> Option<&str> {
        match self {
            Item::Audio(audio) => audio.user.as_ref().map(|user| user.id.as_str()),
            Item::Stream(stream) => stream.user.as_ref().map(|user| user.id.as_str()),
            Item::Comment(comment, _) => Some(&comment.user.id),
            Item::Chat(chat) => Some(&chat.user.id),
            Item::Notification(notification) => {
                notification.actor.as_ref().map(|user| user.id.as_str())
            }
        }
    }

    fn label(&self) -> String {
        match self {
            Item::Audio(audio) => format!(
                "{}, by {}, {}, {} plays, {} favorites{}",
                audio.title,
                name_of(&audio.user),
                ago(audio.created_at),
                audio.plays,
                audio.favorite_count,
                if audio.is_favorited == Some(true) {
                    ", in your favorites"
                } else {
                    ""
                }
            ),
            Item::Stream(stream) => format!(
                "Live: {}, by {}, {} listening",
                stream.title,
                name_of(&stream.user),
                stream.active_listeners
            ),
            Item::Comment(comment, depth) => format!(
                "{}{}: {}, {}",
                if *depth > 0 { "Reply, " } else { "" },
                comment.user.display_name,
                comment.content,
                ago(comment.created_at)
            ),
            Item::Chat(chat) => format!("{}: {}", chat.user.display_name, chat.content),
            Item::Notification(notification) => notification_label(notification),
        }
    }
}

fn notification_label(n: &Notification) -> String {
    let actor = name_of(&n.actor);
    let title = n.target["title"].as_str().unwrap_or("something");
    let text = match (n.kind.as_str(), n.target_type.as_str()) {
        ("comment", _) => format!(
            "{actor} commented on {}: {}",
            n.target["audio"]["title"].as_str().unwrap_or("an audio"),
            n.target["content"].as_str().unwrap_or_default()
        ),
        ("favorite", _) => format!("{actor} favorited {title}"),
        ("upload", "stream") => format!("{actor} went live: {title}"),
        ("upload", _) => format!("{actor} uploaded {title}"),
        _ => format!(
            "{}: {}",
            n.metadata["title"].as_str().unwrap_or("System"),
            n.metadata["content"].as_str().unwrap_or_default()
        ),
    };
    format!(
        "{text}, {}{}",
        ago(n.created_at),
        if n.read_at.is_none() { ", new" } else { "" }
    )
}

#[derive(Default)]
pub struct State {
    pub items: Vec<Item>,
    pub loaded: bool,
    loading: bool,
    error: Option<String>,
    next_page: u32,
    has_more: bool,
    /// The audio an audio page is about.
    audio: Option<Audio>,
    /// The person a profile page is about.
    profile: Option<Profile>,
    /// Whether the signed-in user subscribes to the uploader or profile.
    subscribed: bool,
    /// The comment being replied to: its id and its author's name.
    reply_to: Option<(String, String)>,
}

pub struct Page {
    pub id: u64,
    pub tab: usize,
    pub kind: Kind,
    pub panel: Panel,
    pub list: ListBox,
    details: Option<TextCtrl>,
    pub input: Option<TextCtrl>,
    favorite: Option<Button>,
    subscribe: Option<Button>,
    more: Option<Button>,
    pub st: RefCell<State>,
}

impl Page {
    fn selected(&self) -> Option<usize> {
        let index = self.list.get_selection()? as usize;
        (index < self.st.borrow().items.len()).then_some(index)
    }

    fn selected_item(&self) -> Option<Item> {
        self.selected()
            .and_then(|index| self.st.borrow().items.get(index).cloned())
    }

    /// The row a screen reader finds in an empty list, which it would
    /// otherwise announce as "unknown".
    fn placeholder(&self) -> String {
        let st = self.st.borrow();
        if st.loading {
            return "Loading".to_string();
        }
        if let Some(error) = &st.error {
            return error.clone();
        }
        match self.kind {
            Kind::Feed(_) => "Nothing here yet",
            Kind::Live => "Nobody is live right now",
            Kind::Notifications => "No notifications",
            Kind::Audio(_) => "No comments yet",
            Kind::Room(_) => "No messages yet",
        }
        .to_string()
    }

    /// Rebuilds the list's rows, keeping the selection on the same item.
    fn refill(&self, keep: Option<String>) {
        let (mut labels, selection) = {
            let st = self.st.borrow();
            let labels: Vec<String> = st.items.iter().map(Item::label).collect();
            let kept = keep.and_then(|id| st.items.iter().position(|item| item.id() == id));
            // Selecting a row in a list that does not have focus makes a
            // screen reader read that row out of the blue.
            let first = (!st.items.is_empty() && self.list.has_focus()).then_some(0);
            (labels, kept.or(first))
        };
        if labels.is_empty() {
            labels.push(self.placeholder());
        }
        self.list.clear();
        for label in &labels {
            self.list.append(label);
        }
        if let Some(index) = selection {
            self.list.set_selection(index as u32, true);
        }
    }

    /// Changes an audio wherever this page shows it.
    fn patch_audio(&self, audio_id: &str, change: impl Fn(&mut Audio)) {
        let mut rows = Vec::new();
        {
            let mut st = self.st.borrow_mut();
            if let Some(audio) = st.audio.as_mut().filter(|audio| audio.id == audio_id) {
                change(audio);
            }
            for (index, item) in st.items.iter_mut().enumerate() {
                if let Item::Audio(audio) = item
                    && audio.id == audio_id
                {
                    change(audio);
                    rows.push((index, item.label()));
                }
            }
        }
        for (index, label) in rows {
            self.list.set_string(index as u32, &label);
        }
        self.update_buttons();
    }

    fn update_buttons(&self) {
        let st = self.st.borrow();
        if let (Some(button), Some(audio)) = (&self.favorite, &st.audio) {
            button.set_label(if audio.is_favorited == Some(true) {
                "Remove favorite"
            } else {
                "Favorite"
            });
        }
        if let Some(button) = &self.subscribe {
            button.set_label(if st.subscribed {
                "Unsubscribe"
            } else {
                "Subscribe"
            });
        }
    }
}

fn with_page(id: u64, action: impl FnOnce(&Rc<App>, &Rc<Page>)) {
    if let Some(app) = super::app()
        && let Some(page) = app.find_page(id)
    {
        action(&app, &page);
    }
}

/// Builds a page and shows it on top of `tab`.
pub fn open(app: &Rc<App>, tab: usize, kind: Kind) -> Rc<Page> {
    let id = app.new_page_id();
    let panel = Panel::builder(&app.w.tab_panels[tab]).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = |text: &str| {
        let label = StaticText::builder(&panel).with_label(text).build();
        sizer.add(
            &label,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
            6,
        );
    };
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    let button = |text: &str, command: Id| {
        let button = Button::builder(&panel).with_label(text).build();
        button.on_click(move |_| {
            if let Some(app) = super::app() {
                dispatch(&app, command);
            }
        });
        row.add(&button, 0, SizerFlag::All, 4);
        button
    };

    let has_details = matches!(
        kind,
        Kind::Audio(_) | Kind::Room(_) | Kind::Feed(Source::User(_))
    );
    let details = has_details.then(|| {
        label("Details");
        let details = TextCtrl::builder(&panel)
            .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
            .build();
        acc::name(&details, "Details");
        sizer.add(&details, 1, SizerFlag::Expand | SizerFlag::All, 6);
        details
    });

    let (mut favorite, mut subscribe, mut more) = (None, None, None);
    let list_name = match &kind {
        Kind::Audio(_) => {
            button("Play or pause", ID_PAGE_PLAY);
            favorite = Some(button("Favorite", ID_PAGE_FAVORITE));
            more = Some(button("More", ID_PAGE_MORE));
            "Comments".to_string()
        }
        Kind::Room(_) => {
            button("Play or stop the stream's audio", ID_PAGE_PLAY);
            button("Leave stream", ID_LEAVE);
            "Chat".to_string()
        }
        Kind::Feed(Source::User(_)) => {
            subscribe = Some(button("Subscribe", ID_PAGE_SUBSCRIBE));
            "Uploads".to_string()
        }
        Kind::Feed(Source::Home) => "Audios".to_string(),
        Kind::Feed(Source::Subscriptions) => "From your subscriptions".to_string(),
        Kind::Feed(Source::Favorites) => "Your favorites".to_string(),
        Kind::Feed(Source::Search(text)) => format!("Results for {text}"),
        Kind::Live => "Live streams".to_string(),
        Kind::Notifications => "Notifications".to_string(),
    };
    sizer.add_sizer(&row, 0, SizerFlag::Left | SizerFlag::Right, 2);

    if matches!(kind, Kind::Feed(Source::Home)) {
        label("Sort by");
        let sort = Choice::builder(&panel)
            .with_choices(
                crate::api::SORTS
                    .iter()
                    .map(|(name, _, _)| name.to_string())
                    .collect(),
            )
            .with_selection(Some(app.cfg.borrow().sort.min(5)))
            .build();
        acc::name(&sort, "Sort by");
        sizer.add(&sort, 0, SizerFlag::All, 6);
        sort.on_selection_changed(move |event| {
            let chosen = event.get_selection().unwrap_or(0).max(0) as u32;
            with_page(id, |app, page| {
                app.cfg.borrow_mut().sort = chosen;
                load(app, page, false);
            });
        });
    }

    // The label must be created immediately before the list: it is where the
    // list gets its accessible name.
    label(&list_name);
    let list = ListBox::builder(&panel).build();
    acc::native_list(&list);
    sizer.add(&list, 2, SizerFlag::Expand | SizerFlag::All, 6);
    list.on_item_double_clicked(move |_| with_page(id, activate));
    list.on_selection_changed(move |_| with_page(id, load_more_at_end));
    // The request travels up from the list; only panels can listen for it.
    panel.on_context_menu(move |_| with_page(id, show_item_menu));

    let input = match &kind {
        Kind::Audio(_) => Some(("Write a comment", TextCtrlStyle::MultiLine)),
        Kind::Room(_) => Some(("Write a message", TextCtrlStyle::Default)),
        _ => None,
    }
    .map(|(name, style)| {
        label(name);
        let input = TextCtrl::builder(&panel).with_style(style).build();
        acc::name(&input, name);
        sizer.add(&input, 0, SizerFlag::Expand | SizerFlag::All, 6);
        input
    });
    panel.set_sizer(sizer, true);

    let focus_details = matches!(kind, Kind::Audio(_));
    let page = Rc::new(Page {
        id,
        tab,
        kind,
        panel,
        list,
        details,
        input,
        favorite,
        subscribe,
        more,
        st: RefCell::new(State::default()),
    });
    page.refill(None);
    let visible = tab == app.current_tab();
    app.push(page.clone());
    if visible {
        ensure_loaded(app, &page);
        match &page.details {
            Some(details) if focus_details => details.set_focus(),
            _ => page.list.set_focus(),
        }
    }
    page
}

pub fn ensure_loaded(app: &Rc<App>, page: &Rc<Page>) {
    if !page.st.borrow().loaded {
        load(app, page, false);
    }
}

/// Marks every list of audios out of date, as after posting one. A list
/// that is showing reloads now; the rest reload when next shown.
pub fn refresh_feeds(app: &Rc<App>) {
    for stack in &app.tabs {
        for page in stack.borrow().iter() {
            if matches!(page.kind, Kind::Feed(_)) {
                page.st.borrow_mut().loaded = false;
            }
        }
    }
    if let Some(page) = app.current() {
        ensure_loaded(app, &page);
    }
}

fn flatten(comments: Vec<Comment>, depth: usize, into: &mut Vec<Item>) {
    for mut comment in comments {
        let replies = comment.replies.take().unwrap_or_default();
        into.push(Item::Comment(comment, depth));
        flatten(replies, depth + 1, into);
    }
}

/// Finishes a load: records the outcome on the page if it is still open.
fn finish<T>(app: &Rc<App>, id: u64, result: Result<T>, apply: impl FnOnce(&Rc<Page>, T)) {
    let Some(page) = app.find_page(id) else {
        return;
    };
    {
        let mut st = page.st.borrow_mut();
        st.loading = false;
        st.loaded = true;
        st.error = result.as_ref().err().cloned();
    }
    match result {
        Ok(value) => apply(&page, value),
        Err(error) => {
            page.refill(None);
            if app.current().is_some_and(|current| current.id == id) {
                app.notify(&error);
            }
        }
    }
}

/// Loads a page from the site: from the start, or the next page of a feed.
pub fn load(app: &Rc<App>, page: &Rc<Page>, more: bool) {
    if matches!(page.kind, Kind::Room(_))
        || std::mem::replace(&mut page.st.borrow_mut().loading, true)
    {
        return;
    }
    let id = page.id;
    let keep = page.selected_item().map(|item| item.id().to_string());
    if page.st.borrow().items.is_empty() {
        page.refill(None);
    }
    match &page.kind {
        Kind::Feed(source) => {
            let source = source.clone();
            let sort = app.cfg.borrow().sort as usize;
            let number = if more { page.st.borrow().next_page } else { 1 };
            background(
                app,
                move |api| api.listing(&source, sort, number),
                move |app, result| {
                    finish(app, id, result, |page, listing| {
                        app.set_user(listing.user);
                        let added = {
                            let mut st = page.st.borrow_mut();
                            if !more {
                                st.items.clear();
                            }
                            let before = st.items.len();
                            st.items
                                .extend(listing.streams.into_iter().map(Item::Stream));
                            st.items.extend(listing.audios.into_iter().map(Item::Audio));
                            st.has_more = listing.has_more;
                            st.next_page = number + 1;
                            if let Some(profile) = listing.profile {
                                st.subscribed = profile.is_subscribed;
                                st.profile = Some(profile);
                            }
                            st.items[before..]
                                .iter()
                                .map(Item::label)
                                .collect::<Vec<_>>()
                        };
                        if more {
                            for label in &added {
                                page.list.append(label);
                            }
                            app.notify(&format!("{} more", added.len()));
                        } else {
                            page.refill(keep);
                            show_profile(page);
                        }
                    });
                },
            );
        }
        Kind::Live => background(
            app,
            |api| api.listing(&Source::Home, 0, 1),
            move |app, result| {
                finish(app, id, result, |page, listing| {
                    app.set_user(listing.user);
                    page.st.borrow_mut().items =
                        listing.streams.into_iter().map(Item::Stream).collect();
                    page.refill(keep);
                });
            },
        ),
        Kind::Notifications => background(
            app,
            |api| api.notifications(),
            move |app, result| {
                finish(app, id, result, |page, notifications| {
                    page.st.borrow_mut().items =
                        notifications.into_iter().map(Item::Notification).collect();
                    page.refill(keep);
                    // Loading them marks them all read on the site.
                    set_tab_count(app, TAB_NOTIFICATIONS, 0);
                });
            },
        ),
        Kind::Audio(audio_id) => {
            let audio_id = audio_id.clone();
            background(
                app,
                move |api| api.audio(&audio_id),
                move |app, result| {
                    finish(app, id, result, |page, loaded| {
                        let audio = loaded.audio;
                        let text = format!(
                            "{}\r\nBy {}\r\nUploaded {}. {} plays. {} favorites.\r\n\r\n{}",
                            audio.title,
                            name_of(&audio.user),
                            ago(audio.created_at),
                            audio.plays,
                            audio.favorite_count,
                            audio.description
                        );
                        let heading = format!("{}, by {}", audio.title, name_of(&audio.user));
                        {
                            let mut st = page.st.borrow_mut();
                            st.items.clear();
                            flatten(loaded.comments, 0, &mut st.items);
                            st.audio = Some(audio);
                            st.subscribed = loaded.is_subscribed;
                        }
                        if let Some(details) = &page.details {
                            details.set_value(&text);
                        }
                        page.update_buttons();
                        page.refill(keep);
                        if app.current().is_some_and(|current| current.id == id) {
                            app.notify(&heading);
                        }
                    });
                },
            );
        }
        Kind::Room(_) => {}
    }
}

fn show_profile(page: &Page) {
    let text = {
        let st = page.st.borrow();
        let Some(profile) = &st.profile else { return };
        let live = match &profile.stream {
            Some(stream) => format!("Live now: {}\r\n", stream.title),
            None => String::new(),
        };
        format!(
            "{}\r\n@{}\r\n{} subscribers\r\n{live}\r\n{}",
            profile.user.display_name, profile.user.name, profile.subscribers, profile.user.bio
        )
    };
    if let Some(details) = &page.details {
        details.set_value(&text);
    }
    page.update_buttons();
}

fn load_more_at_end(app: &Rc<App>, page: &Rc<Page>) {
    let at_end = {
        let st = page.st.borrow();
        st.has_more && !st.loading && page.selected() == Some(st.items.len().saturating_sub(1))
    };
    if at_end && matches!(page.kind, Kind::Feed(_)) {
        app.notify("Loading more");
        load(app, page, true);
    }
}

/// Fills a stream's room with what was fetched when joining it.
pub fn show_room(page: &Page, stream: &Stream, chats: Vec<Chat>) {
    if let Some(details) = &page.details {
        details.set_value(&format!(
            "{}\r\nLive stream by {}\r\n\r\n{}",
            stream.title,
            name_of(&stream.user),
            stream.description
        ));
    }
    {
        let mut st = page.st.borrow_mut();
        st.loaded = true;
        st.items = chats.into_iter().map(Item::Chat).collect();
    }
    page.refill(None);
}

pub fn append_chat(page: &Page, chat: Chat) {
    let item = Item::Chat(chat);
    let label = item.label();
    let was_empty = {
        let mut st = page.st.borrow_mut();
        st.items.push(item);
        st.items.len() == 1
    };
    if was_empty {
        page.list.clear();
    }
    page.list.append(&label);
}

pub fn remove_item(page: &Page, item_id: &str) {
    let index = page
        .st
        .borrow()
        .items
        .iter()
        .position(|item| item.id() == item_id);
    if let Some(index) = index {
        page.st.borrow_mut().items.remove(index);
        if page.st.borrow().items.is_empty() {
            page.refill(None);
        } else {
            page.list.delete(index as u32);
        }
    }
}

/// Enter on a row: the main thing to do with it.
pub fn activate(app: &Rc<App>, page: &Rc<Page>) {
    match page.selected_item() {
        Some(Item::Audio(audio)) => {
            open(app, page.tab, Kind::Audio(audio.id));
        }
        Some(Item::Stream(stream)) => live::join(app, &stream.id),
        Some(Item::Comment(comment, _)) => {
            if let Some(input) = &page.input {
                app.notify(&format!(
                    "Replying to {}. Escape cancels.",
                    comment.user.display_name
                ));
                page.st.borrow_mut().reply_to = Some((comment.id, comment.user.display_name));
                input.set_focus();
            }
        }
        Some(Item::Notification(n)) => {
            let comment_audio = n.target["audio"]["id"]
                .as_str()
                .or(n.metadata["audioId"].as_str());
            match (
                n.target_type.as_str(),
                comment_audio,
                n.target["id"].as_str(),
            ) {
                ("comment", Some(audio_id), _) => {
                    open(app, page.tab, Kind::Audio(audio_id.to_string()));
                }
                ("audio", _, Some(audio_id)) => {
                    open(app, page.tab, Kind::Audio(audio_id.to_string()));
                }
                ("stream", _, Some(stream_id)) => live::join(app, stream_id),
                _ => app.notify("There is nothing to open for this notification"),
            }
        }
        Some(Item::Chat(_)) | None => {}
    }
}

/// Space on a row.
pub fn play_selected(app: &Rc<App>, page: &Rc<Page>) {
    match page.selected_item() {
        Some(Item::Audio(audio)) => playback::play_or_toggle(app, audio),
        Some(Item::Stream(stream)) => live::join(app, &stream.id),
        _ => playback::toggle(app),
    }
}

/// Escape while replying: stops replying. Returns whether there was a reply.
pub fn cancel_reply(app: &App, page: &Page) -> bool {
    let was_replying = page.st.borrow_mut().reply_to.take().is_some();
    if was_replying {
        app.notify("Reply cancelled");
        page.list.set_focus();
    }
    was_replying
}

/// Enter in the text field: posts the comment or sends the chat message.
pub fn submit(app: &Rc<App>, page: &Rc<Page>) {
    let Some(input) = &page.input else { return };
    let text = input.get_value().trim().to_string();
    if text.is_empty() {
        return;
    }
    let id = page.id;
    match &page.kind {
        Kind::Audio(audio_id) => {
            let audio_id = audio_id.clone();
            let parent = page
                .st
                .borrow()
                .reply_to
                .as_ref()
                .map(|(comment_id, _)| comment_id.clone());
            background(
                app,
                move |api| api.add_comment(&audio_id, &text, parent.as_deref()),
                move |app, result| match result {
                    Err(error) => app.notify(&error),
                    Ok(()) => with_page(id, |app, page| {
                        if let Some(input) = &page.input {
                            input.set_value("");
                        }
                        page.st.borrow_mut().reply_to = None;
                        app.notify("Comment posted");
                        load(app, page, false);
                    }),
                },
            );
        }
        Kind::Room(stream_id) => {
            let stream_id = stream_id.clone();
            input.set_value("");
            background(
                app,
                move |api| api.send_chat(&stream_id, &text),
                |app, result| {
                    if let Err(error) = result {
                        app.notify(&error);
                    }
                },
            );
        }
        _ => {}
    }
}

/// The Applications key or Shift+F10: everything the selected row can do.
fn show_item_menu(app: &Rc<App>, page: &Rc<Page>) {
    let Some(item) = page.selected_item().filter(|_| page.list.has_focus()) else {
        return;
    };
    let menu = Menu::builder();
    let mut menu = match &item {
        Item::Audio(audio) => menu
            .append_item(ID_ITEM_PLAY, "&Play or pause\tSpace", "")
            .append_item(ID_ITEM_PLAY_ALL, "Play all &from here", "")
            .append_item(ID_ITEM_OPEN, "&Open\tEnter", "")
            .append_item(
                ID_ITEM_FAVORITE,
                if audio.is_favorited == Some(true) {
                    "Remove fa&vorite"
                } else {
                    "Fa&vorite"
                },
                "",
            )
            .append_item(ID_ITEM_COPY_LINK, "Copy &link", "")
            .append_item(ID_ITEM_PROFILE, "View p&rofile", ""),
        Item::Stream(_) => menu
            .append_item(ID_ITEM_OPEN, "&Join\tEnter", "")
            .append_item(ID_ITEM_COPY_LINK, "Copy &link", "")
            .append_item(ID_ITEM_PROFILE, "View p&rofile", ""),
        Item::Comment(comment, _) => {
            let menu = menu
                .append_item(ID_ITEM_OPEN, "&Reply\tEnter", "")
                .append_item(ID_ITEM_COPY_TEXT, "&Copy text", "")
                .append_item(ID_ITEM_PROFILE, "View p&rofile", "");
            let mine = app
                .user
                .borrow()
                .as_ref()
                .is_some_and(|user| user.id == comment.user.id);
            if mine {
                menu.append_item(ID_ITEM_DELETE, "&Delete\tDelete", "")
            } else {
                menu
            }
        }
        Item::Chat(_) => menu
            .append_item(ID_ITEM_COPY_TEXT, "&Copy text", "")
            .append_item(ID_ITEM_PROFILE, "View p&rofile", ""),
        Item::Notification(_) => menu
            .append_item(ID_ITEM_OPEN, "&Open\tEnter", "")
            .append_item(ID_ITEM_DELETE, "&Delete\tDelete", ""),
    }
    .build();
    page.list.popup_menu(&mut menu, None);
}

fn show_more_menu(page: &Page) {
    let (Some(button), Some(uploader)) = (
        &page.more,
        page.st
            .borrow()
            .audio
            .as_ref()
            .map(|a| name_of(&a.user).to_string()),
    ) else {
        return;
    };
    let subscribe = if page.st.borrow().subscribed {
        format!("Un&subscribe from {uploader}")
    } else {
        format!("&Subscribe to {uploader}")
    };
    let mut menu = Menu::builder()
        .append_item(ID_PAGE_SUBSCRIBE, &subscribe, "")
        .append_item(ID_PAGE_PROFILE, "View p&rofile", "")
        .append_item(ID_PAGE_COPY_LINK, "Copy &link", "")
        .append_item(ID_PAGE_BROWSER, "Open in your &browser", "")
        .build();
    button.popup_menu(&mut menu, None);
}

fn toggle_favorite(app: &Rc<App>, page: &Page, audio: Audio) {
    let favorite = audio.is_favorited != Some(true);
    let id = page.id;
    background(
        app,
        {
            let audio_id = audio.id.clone();
            move |api| api.set_favorite(&audio_id, favorite)
        },
        move |app, result| match result {
            Err(error) => app.notify(&error),
            Ok(()) => {
                app.notify(if favorite {
                    "Added to your favorites"
                } else {
                    "Removed from your favorites"
                });
                with_page(id, |_, page| {
                    page.patch_audio(&audio.id, |audio| {
                        audio.is_favorited = Some(favorite);
                        audio.favorite_count += if favorite { 1 } else { -1 };
                    });
                });
            }
        },
    );
}

fn toggle_subscribe(app: &Rc<App>, page: &Page) {
    let (path, name) = match (&page.kind, &*page.st.borrow()) {
        (
            Kind::Audio(audio_id),
            State {
                audio: Some(audio), ..
            },
        ) => (
            format!("/listen/{audio_id}"),
            name_of(&audio.user).to_string(),
        ),
        (
            Kind::Feed(Source::User(user_id)),
            State {
                profile: Some(profile),
                ..
            },
        ) => (
            format!("/user/{user_id}"),
            profile.user.display_name.clone(),
        ),
        _ => return,
    };
    let subscribe = !page.st.borrow().subscribed;
    let id = page.id;
    background(
        app,
        move |api| api.set_subscribed(&path, subscribe),
        move |app, result| match result {
            Err(error) => app.notify(&error),
            Ok(()) => {
                let verb = if subscribe {
                    "Subscribed to"
                } else {
                    "Unsubscribed from"
                };
                app.notify(&format!("{verb} {name}"));
                with_page(id, |_, page| {
                    page.st.borrow_mut().subscribed = subscribe;
                    page.update_buttons();
                });
            }
        },
    );
}

fn delete_selected(app: &Rc<App>, page: &Rc<Page>) {
    let id = page.id;
    match (page.selected_item(), &page.kind) {
        (Some(Item::Comment(comment, _)), Kind::Audio(audio_id)) => {
            if app
                .user
                .borrow()
                .as_ref()
                .is_none_or(|user| user.id != comment.user.id)
            {
                app.notify("You can only delete your own comments");
            } else if dialogs::confirm(app, "Delete comment", "Delete this comment?") {
                let audio_id = audio_id.clone();
                background(
                    app,
                    move |api| api.delete_comment(&audio_id, &comment.id),
                    move |app, result| match result {
                        Err(error) => app.notify(&error),
                        Ok(()) => with_page(id, |app, page| {
                            app.notify("Comment deleted");
                            load(app, page, false);
                        }),
                    },
                );
            }
        }
        (Some(Item::Notification(notification)), _) => {
            let notification_id = notification.id.clone();
            background(
                app,
                move |api| api.delete_notification(&notification_id),
                move |app, result| match result {
                    Err(error) => app.notify(&error),
                    Ok(()) => with_page(id, |app, page| {
                        remove_item(page, &notification.id);
                        app.notify("Notification deleted");
                    }),
                },
            );
        }
        _ => {}
    }
}

/// A command on the selected row, or on the page's own buttons.
pub fn command(app: &Rc<App>, page: &Rc<Page>, id: Id) {
    let item = page.selected_item();
    let page_audio = page.st.borrow().audio.clone();
    let link = |path: &str, id: &str| format!("{}/{path}/{id}", app.api.base);
    let profile = |user_id: Option<&str>| {
        if let Some(user_id) = user_id {
            open(app, page.tab, Kind::Feed(Source::User(user_id.to_string())));
        }
    };
    match id {
        ID_ITEM_PLAY => play_selected(app, page),
        ID_ITEM_PLAY_ALL => {
            let from = page.selected().unwrap_or(0);
            let audios = page.st.borrow().items[from..]
                .iter()
                .filter_map(|item| match item {
                    Item::Audio(audio) => Some(audio.clone()),
                    _ => None,
                })
                .collect();
            playback::play_queue(app, audios);
        }
        ID_ITEM_OPEN => activate(app, page),
        ID_ITEM_FAVORITE => {
            if let Some(Item::Audio(audio)) = item {
                toggle_favorite(app, page, audio);
            }
        }
        ID_ITEM_COPY_LINK => match &item {
            Some(Item::Audio(audio)) => copy_to_clipboard(app, &link("listen", &audio.id), "Link"),
            Some(Item::Stream(stream)) => copy_to_clipboard(app, &link("live", &stream.id), "Link"),
            _ => {}
        },
        ID_ITEM_PROFILE => profile(item.as_ref().and_then(Item::author_id)),
        ID_ITEM_COPY_TEXT => match &item {
            Some(Item::Comment(comment, _)) => copy_to_clipboard(app, &comment.content, "Comment"),
            Some(Item::Chat(chat)) => copy_to_clipboard(app, &chat.content, "Message"),
            _ => {}
        },
        ID_ITEM_DELETE => delete_selected(app, page),
        ID_PAGE_PLAY => match (&page.kind, page_audio) {
            (Kind::Audio(_), Some(audio)) => playback::play_or_toggle(app, audio),
            (Kind::Room(_), _) => live::toggle_audio(app),
            _ => {}
        },
        ID_PAGE_FAVORITE => {
            if let Some(audio) = page_audio {
                toggle_favorite(app, page, audio);
            }
        }
        ID_PAGE_MORE => show_more_menu(page),
        ID_PAGE_SUBSCRIBE => toggle_subscribe(app, page),
        ID_PAGE_PROFILE => profile(
            page_audio
                .as_ref()
                .and_then(|audio| audio.user.as_ref())
                .map(|user| user.id.as_str()),
        ),
        ID_PAGE_COPY_LINK => {
            if let Some(audio) = page_audio {
                copy_to_clipboard(app, &link("listen", &audio.id), "Link");
            }
        }
        ID_PAGE_BROWSER => {
            if let Some(audio) = page_audio {
                open_in_browser(&link("listen", &audio.id));
            }
        }
        _ => {}
    }
}

/// Shows how many notifications are unread on the Notifications tab.
pub fn poll_unread(app: &Rc<App>) {
    if app.api.token().is_some() {
        background(
            app,
            |api| api.unread_notifications(),
            |app, result| {
                if let Ok(count) = result {
                    set_tab_count(app, TAB_NOTIFICATIONS, count);
                }
            },
        );
    }
}
