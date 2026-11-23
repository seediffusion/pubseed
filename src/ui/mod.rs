//! The main window. Its layout follows the reference app: a menu bar holding
//! every command, four tabs whose pages open in place and go back with
//! Escape, a player along the bottom, and a status bar.

pub mod composer;
pub mod dialogs;
pub mod live;
pub mod pages;
pub mod playback;

use crate::api::{Api, Audio, Source, Stream, User};
use crate::config::{Config, SpeechOutput};
use crate::player::Player;
use crate::speech;
use pages::{Kind, Page};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use wxdragon::prelude::*;

pub const TAB_HOME: usize = 0;
pub const TAB_LIVE: usize = 2;
pub const TAB_NOTIFICATIONS: usize = 3;
const TAB_NAMES: [&str; 4] = ["Home", "Subscriptions", "Live", "Notifications"];

pub const ID_SIGN_IN: Id = 6001;
pub const ID_SIGN_OUT: Id = 6002;
pub const ID_SETTINGS: Id = 6003;
pub const ID_EXIT: Id = 6004;
pub const ID_NEW_AUDIO: Id = 6005;
pub const ID_TAB_FIRST: Id = 6010; // ..=6013
pub const ID_REFRESH: Id = 6014;
pub const ID_BACK: Id = 6020;
pub const ID_PROFILE: Id = 6021;
pub const ID_FAVORITES: Id = 6022;
pub const ID_SEARCH: Id = 6023;
pub const ID_NEXT_REGION: Id = 6024;
pub const ID_PREVIOUS_REGION: Id = 6025;
pub const ID_PLAY_PAUSE: Id = 6030;
pub const ID_STOP: Id = 6031;
pub const ID_SKIP_BACK: Id = 6032;
pub const ID_SKIP_FORWARD: Id = 6033;
pub const ID_QUEUE_PREVIOUS: Id = 6034;
pub const ID_QUEUE_NEXT: Id = 6035;
pub const ID_VOLUME_UP: Id = 6036;
pub const ID_VOLUME_DOWN: Id = 6037;
pub const ID_WHATS_PLAYING: Id = 6038;
pub const ID_FASTER: Id = 6060;
pub const ID_SLOWER: Id = 6061;
pub const ID_SPEED_FIRST: Id = 6062; // ..=6068, one for each of playback::SPEEDS
pub const ID_READ_CHAT: Id = 6040;
pub const ID_LISTENERS: Id = 6041;
pub const ID_LEAVE: Id = 6042;
pub const ID_WEBSITE: Id = 6050;
pub const ID_ABOUT: Id = 6051;
// The selected row's menu.
pub const ID_ITEM_PLAY: Id = 6100;
pub const ID_ITEM_PLAY_ALL: Id = 6101;
pub const ID_ITEM_OPEN: Id = 6102;
pub const ID_ITEM_FAVORITE: Id = 6103;
pub const ID_ITEM_COPY_LINK: Id = 6104;
pub const ID_ITEM_PROFILE: Id = 6105;
pub const ID_ITEM_COPY_TEXT: Id = 6106;
pub const ID_ITEM_DELETE: Id = 6107;
pub const ID_ITEM_EDIT: Id = 6108;
pub const ID_ITEM_CLEAR_ALL: Id = 6109;
pub const ID_ITEM_GO_TO_PARENT: Id = 6110;
// The open page's buttons and More menu.
pub const ID_PAGE_PLAY: Id = 6120;
pub const ID_PAGE_FAVORITE: Id = 6121;
pub const ID_PAGE_MORE: Id = 6122;
pub const ID_PAGE_SUBSCRIBE: Id = 6123;
pub const ID_PAGE_PROFILE: Id = 6124;
pub const ID_PAGE_COPY_LINK: Id = 6125;
pub const ID_PAGE_BROWSER: Id = 6126;
pub const ID_PAGE_EDIT: Id = 6127;
pub const ID_PAGE_DELETE: Id = 6128;
pub const ID_PAGE_COMMENT_NOTIFICATIONS: Id = 6129;

pub enum Now {
    Audio(Audio),
    Live(Stream),
}

pub struct Widgets {
    pub frame: Frame,
    pub notebook: Notebook,
    pub tab_panels: [Panel; 4],
    pub tab_sizers: [BoxSizer; 4],
    /// The panel everything in the window sits on.
    pub root: Panel,
    /// The player along the bottom, shown only while something is in it.
    pub player_bar: Panel,
    pub play_button: Button,
    pub position: Slider,
    pub volume: Slider,
    pub status: StatusBar,
}

pub struct App {
    pub api: Arc<Api>,
    pub cfg: RefCell<Config>,
    pub user: RefCell<Option<User>>,
    pub player: RefCell<Player>,
    pub now: RefCell<Option<Now>>,
    /// The audios "Play all from here" is working through, and where it is.
    pub queue: RefCell<(Vec<Audio>, usize)>,
    pub room: RefCell<Option<live::Room>>,
    /// The New audio window, while it is open.
    pub composer: RefCell<Option<Rc<composer::Composer>>>,
    /// Each tab's pages, the visible one last.
    pub tabs: [RefCell<Vec<Rc<Page>>>; 4],
    pub w: Widgets,
    next_page_id: Cell<u64>,
    /// How many of our own modal dialogs are open.
    modal: Cell<u32>,
    ticks: Cell<u32>,
    timer: RefCell<Option<Timer<Frame>>>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

pub fn app() -> Option<Rc<App>> {
    APP.with(|slot| slot.borrow().clone())
}

/// Runs `work` on a worker thread, then `done` with its result on the UI thread.
pub fn background<T: Send + 'static>(
    app: &App,
    work: impl FnOnce(&Api) -> T + Send + 'static,
    done: impl FnOnce(&Rc<App>, T) + Send + 'static,
) {
    let api = app.api.clone();
    std::thread::spawn(move || {
        let result = work(&api);
        wxdragon::call_after(Box::new(move || {
            if let Some(app) = self::app() {
                done(&app, result);
            }
        }));
        // The queue is drained when the event loop goes idle.
        wxdragon::app::wake_up_idle();
    });
}

/// "3 hours ago", from a millisecond timestamp.
pub fn ago(timestamp_ms: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let minutes = (now - timestamp_ms).max(0) / 60_000;
    let (count, unit) = match minutes {
        0 => return "just now".to_string(),
        1..60 => (minutes, "minute"),
        60..1440 => (minutes / 60, "hour"),
        1440..43_200 => (minutes / 1440, "day"),
        43_200..525_600 => (minutes / 43_200, "month"),
        _ => (minutes / 525_600, "year"),
    };
    format!("{count} {unit}{} ago", if count == 1 { "" } else { "s" })
}

impl App {
    /// Tells the user something: in the status bar, and through the screen reader.
    pub fn notify(&self, text: &str) {
        self.w.status.set_status_text(text, 0);
        speech::announce(text, true);
    }

    /// Reads a chat message with whichever output Settings chose.
    pub fn speak(&self, text: &str) {
        let cfg = self.cfg.borrow();
        match cfg.speech_output {
            SpeechOutput::ScreenReader => speech::announce(text, false),
            SpeechOutput::Voice => speech::say(&cfg.voice, cfg.voice_rate, text, false),
        }
    }

    pub fn new_page_id(&self) -> u64 {
        self.next_page_id.set(self.next_page_id.get() + 1);
        self.next_page_id.get()
    }

    pub fn current_tab(&self) -> usize {
        (self.w.notebook.selection().max(0) as usize).min(3)
    }

    pub fn top(&self, tab: usize) -> Option<Rc<Page>> {
        self.tabs[tab].borrow().last().cloned()
    }

    pub fn current(&self) -> Option<Rc<Page>> {
        self.top(self.current_tab())
    }

    pub fn find_page(&self, id: u64) -> Option<Rc<Page>> {
        self.tabs
            .iter()
            .find_map(|stack| stack.borrow().iter().find(|page| page.id == id).cloned())
    }

    /// Shows `page` in its tab, over the page that was there.
    pub fn push(&self, page: Rc<Page>) {
        let tab = page.tab;
        if let Some(previous) = self.top(tab) {
            previous.panel.hide();
        }
        self.w.tab_sizers[tab].add(&page.panel, 1, SizerFlag::Expand, 0);
        self.tabs[tab].borrow_mut().push(page);
        self.w.tab_panels[tab].layout();
    }

    /// Closes a page. If it was the one showing, the page under it shows again.
    pub fn remove_page(&self, id: u64) {
        let Some(page) = self.find_page(id) else {
            return;
        };
        let tab = page.tab;
        let was_top = self.top(tab).is_some_and(|top| top.id == id);
        self.tabs[tab].borrow_mut().retain(|page| page.id != id);
        page.panel.hide();
        if was_top && let Some(previous) = self.top(tab) {
            previous.panel.show(true);
            self.w.tab_panels[tab].layout();
            if tab == self.current_tab() {
                previous.list.set_focus();
            }
        }
        // This usually runs inside one of the page's own event handlers, so
        // the window is destroyed once that handler has returned.
        let panel = page.panel;
        wxdragon::call_after(Box::new(move || panel.destroy()));
    }

    pub fn back(self: &Rc<Self>) {
        let tab = self.current_tab();
        let Some(page) = self.top(tab) else { return };
        if pages::cancel_reply(self, &page) {
            return;
        }
        if matches!(page.kind, Kind::Room(_)) {
            self.notify("To leave the stream, use Live, Leave stream, or press Ctrl+Shift+L.");
        } else if self.tabs[tab].borrow().len() <= 1 {
            self.notify("Nothing to go back to");
        } else {
            self.remove_page(page.id);
            // The page underneath may have gone out of date while covered.
            if let Some(revealed) = self.top(tab) {
                pages::ensure_loaded(self, &revealed);
            }
        }
    }

    pub fn select_tab(self: &Rc<Self>, tab: usize) {
        self.w.notebook.set_selection(tab);
        if let Some(page) = self.top(tab) {
            pages::ensure_loaded(self, &page);
            page.list.set_focus();
        }
    }

    pub fn set_user(&self, user: Option<User>) {
        let text = match &user {
            Some(user) => format!("Signed in as {}", user.display_name),
            None => "Not signed in".to_string(),
        };
        self.w.status.set_status_text(&text, 1);
        // Only the one that applies can be chosen; the other is greyed out.
        if let Some(menu_bar) = self.w.frame.get_menu_bar() {
            menu_bar.enable_item(ID_SIGN_IN, user.is_none());
            menu_bar.enable_item(ID_SIGN_OUT, user.is_some());
        }
        *self.user.borrow_mut() = user;
    }

    /// Reloads the visible page and leaves the other tabs to reload when shown.
    pub fn reload_everything(self: &Rc<Self>) {
        for stack in &self.tabs {
            for page in stack.borrow().iter() {
                page.st.borrow_mut().loaded = false;
            }
        }
        if let Some(page) = self.current() {
            pages::ensure_loaded(self, &page);
        }
    }

    /// Runs one of our modal dialogs, so Escape inside it is not taken for Back.
    pub fn modal<T>(&self, show: impl FnOnce() -> T) -> T {
        self.modal.set(self.modal.get() + 1);
        let result = show();
        self.modal.set(self.modal.get() - 1);
        result
    }

    /// F6: moves between the tab strip, the page and the player.
    fn move_region(&self, forward: bool) {
        let w = &self.w;
        let region = if w.notebook.has_focus() {
            0
        } else if w.play_button.has_focus() || w.position.has_focus() || w.volume.has_focus() {
            2
        } else {
            1
        };
        // With nothing in the player there are only two regions.
        let regions = if w.player_bar.is_shown() { 3 } else { 2 };
        match (region + if forward { 1 } else { regions - 1 }) % regions {
            0 => w.notebook.set_focus(),
            1 => {
                if let Some(page) = self.current() {
                    page.list.set_focus();
                }
            }
            _ => w.play_button.set_focus(),
        }
    }
}

fn build_menu_bar(read_chat: bool, speed: f32, skip_seconds: u32) -> MenuBar {
    let file = Menu::builder()
        .append_item(ID_NEW_AUDIO, "&New audio...	Ctrl+N", "")
        .append_separator()
        .append_item(ID_SIGN_IN, "Sign &in...", "")
        .append_item(ID_SIGN_OUT, "Sign &out", "")
        .append_separator()
        .append_item(ID_SETTINGS, "&Settings...\tCtrl+,", "")
        .append_separator()
        .append_item(ID_EXIT, "E&xit\tCtrl+Q", "")
        .build();
    let view = Menu::builder()
        .append_item(ID_TAB_FIRST, "&Home\tCtrl+1", "")
        .append_item(ID_TAB_FIRST + 1, "&Subscriptions\tCtrl+2", "")
        .append_item(ID_TAB_FIRST + 2, "&Live\tCtrl+3", "")
        .append_item(ID_TAB_FIRST + 3, "&Notifications\tCtrl+4", "")
        .append_separator()
        .append_item(ID_REFRESH, "&Refresh\tF5", "")
        .build();
    let go = Menu::builder()
        .append_item(ID_BACK, "&Back\tAlt+Left", "")
        .append_separator()
        .append_item(ID_PROFILE, "Your &profile\tCtrl+U", "")
        .append_item(ID_FAVORITES, "&Favorites\tCtrl+B", "")
        .append_item(ID_SEARCH, "&Search...\tCtrl+F", "")
        .append_separator()
        .append_item(ID_NEXT_REGION, "&Next region\tF6", "")
        .append_item(ID_PREVIOUS_REGION, "P&revious region\tShift+F6", "")
        .build();
    let player = Menu::builder()
        .append_item(ID_PLAY_PAUSE, "&Play or pause\tCtrl+P", "")
        .append_item(ID_STOP, "S&top\tCtrl+.", "")
        .append_separator()
        .append_item(ID_SKIP_BACK, &playback::skip_label(-1, skip_seconds), "")
        .append_item(ID_SKIP_FORWARD, &playback::skip_label(1, skip_seconds), "")
        .append_item(ID_QUEUE_PREVIOUS, "P&revious in queue\tAlt+Shift+PgUp", "")
        .append_item(ID_QUEUE_NEXT, "&Next in queue\tAlt+Shift+PgDn", "")
        .append_separator()
        .append_item(ID_VOLUME_UP, "Volume &up\tAlt+Shift+Up", "")
        .append_item(ID_VOLUME_DOWN, "Volume &down\tAlt+Shift+Down", "")
        .append_separator()
        .append_item(ID_FASTER, "F&aster\tAlt+.", "")
        .append_item(ID_SLOWER, "S&lower\tAlt+,", "")
        .append_separator()
        .append_item(ID_WHATS_PLAYING, "&What's playing\tCtrl+Shift+W", "")
        .build();
    let mut speeds = Menu::builder();
    for (index, choice) in playback::SPEEDS.iter().enumerate() {
        let label = if *choice == 1.0 {
            "1, normal".to_string()
        } else {
            choice.to_string()
        };
        speeds = speeds.append_radio_item(ID_SPEED_FIRST + index as Id, &label, "");
    }
    let speeds = speeds.build();
    let current = playback::SPEEDS
        .iter()
        .position(|choice| (choice - speed).abs() < 0.01)
        .unwrap_or(2);
    speeds.check_item(ID_SPEED_FIRST + current as Id, true);
    player.append_submenu(speeds, "Sp&eed", "");
    let live = Menu::builder()
        .append_check_item(ID_READ_CHAT, "&Read chat aloud\tCtrl+Shift+R", "")
        .append_item(ID_LISTENERS, "&Who's listening\tCtrl+Shift+S", "")
        .append_separator()
        .append_item(ID_LEAVE, "&Leave stream\tCtrl+Shift+L", "")
        .build();
    live.check_item(ID_READ_CHAT, read_chat);
    let help = Menu::builder()
        .append_item(ID_WEBSITE, "Open the AudioPub &website", "")
        .append_item(ID_ABOUT, "&About Pubseed", "")
        .build();
    MenuBar::builder()
        .append(file, "&File")
        .append(view, "&View")
        .append(go, "&Go")
        .append(player, "&Player")
        .append(live, "&Live")
        .append(help, "&Help")
        .build()
}

/// Every menu command, button and shortcut ends up here.
pub fn dispatch(app: &Rc<App>, id: Id) {
    match id {
        ID_NEW_AUDIO => composer::open(app),
        ID_SIGN_IN => dialogs::sign_in(app),
        ID_SIGN_OUT => {
            app.api.sign_out();
            app.cfg.borrow().set_token(None);
            app.set_user(None);
            app.notify("Signed out");
            app.reload_everything();
        }
        ID_SETTINGS => dialogs::settings(app),
        ID_EXIT => {
            app.w.frame.close(false);
        }
        ID_TAB_FIRST..=6013 => app.select_tab((id - ID_TAB_FIRST) as usize),
        ID_REFRESH => {
            if let Some(page) = app.current() {
                pages::load(app, &page, false);
            }
        }
        ID_BACK => app.back(),
        ID_PROFILE => match app.user.borrow().as_ref().map(|user| user.id.clone()) {
            Some(id) => {
                pages::open(app, app.current_tab(), Kind::Feed(Source::User(id)));
            }
            None => app.notify("Sign in to see your profile. Use File, Sign in."),
        },
        ID_FAVORITES if app.user.borrow().is_none() => {
            app.notify("Sign in to see your favorites. Use File, Sign in.");
        }
        ID_FAVORITES => {
            pages::open(app, app.current_tab(), Kind::Feed(Source::Favorites));
        }
        ID_SEARCH => dialogs::search(app),
        ID_NEXT_REGION => app.move_region(true),
        ID_PREVIOUS_REGION => app.move_region(false),
        ID_PLAY_PAUSE => playback::toggle(app),
        ID_STOP => playback::stop_command(app),
        ID_SKIP_BACK => playback::skip(app, -1),
        ID_SKIP_FORWARD => playback::skip(app, 1),
        ID_QUEUE_PREVIOUS => playback::step_queue(app, -1),
        ID_QUEUE_NEXT => playback::step_queue(app, 1),
        ID_VOLUME_UP => playback::change_volume(app, 5),
        ID_VOLUME_DOWN => playback::change_volume(app, -5),
        ID_WHATS_PLAYING => playback::whats_playing(app),
        ID_FASTER => playback::step_speed(app, 1),
        ID_SLOWER => playback::step_speed(app, -1),
        ID_SPEED_FIRST..=6068 => playback::set_speed(app, (id - ID_SPEED_FIRST) as usize),
        ID_READ_CHAT => {
            let on = !app.cfg.borrow().read_chat;
            app.cfg.borrow_mut().read_chat = on;
            if let Some(menu_bar) = app.w.frame.get_menu_bar() {
                menu_bar.check_item(ID_READ_CHAT, on);
            }
            app.notify(if on {
                "Reading chat aloud"
            } else {
                "Not reading chat aloud"
            });
        }
        ID_LISTENERS => live::listeners(app),
        ID_LEAVE => live::leave(app, "Left the stream"),
        ID_WEBSITE => open_in_browser(&app.api.base),
        ID_ABOUT => dialogs::message(
            app,
            "About Pubseed",
            concat!(
                "Pubseed ",
                env!("CARGO_PKG_VERSION"),
                "\nAn accessible desktop client for AudioPub."
            ),
        ),
        _ => {
            if let Some(page) = app.current() {
                pages::command(app, &page, id);
            }
        }
    }
}

pub fn open_in_browser(url: &str) {
    wxdragon::utils::launch_default_browser(url, wxdragon::utils::BrowserLaunchFlags::Default);
}

pub fn copy_to_clipboard(app: &App, text: &str, what: &str) {
    let clipboard = Clipboard::get();
    if clipboard.set_text(text) {
        app.notify(&format!("{what} copied"));
    }
}

/// Keys that act on whatever has focus. Returns whether the key was used.
fn handle_key(app: &Rc<App>, code: i32, shift: bool) -> bool {
    if app.modal.get() > 0 {
        return false;
    }
    let Some(page) = app.current() else {
        return false;
    };
    let enter = code == WXK_RETURN || code == WXK_NUMPAD_ENTER;
    if page.list.has_focus() {
        match code {
            _ if enter => pages::activate(app, &page),
            WXK_SPACE => pages::play_selected(app, &page),
            WXK_LEFT => playback::skip(app, -1),
            WXK_RIGHT => playback::skip(app, 1),
            WXK_DELETE => pages::command(app, &page, ID_ITEM_DELETE),
            WXK_ESCAPE => app.back(),
            _ => return false,
        }
        return true;
    }
    if page.input.as_ref().is_some_and(|input| input.has_focus()) {
        if enter && !shift {
            pages::submit(app, &page);
            return true;
        }
    } else if app.w.position.has_focus() && playback::position_key(app, code) {
        return true;
    }
    if code == WXK_ESCAPE {
        app.back();
        return true;
    }
    false
}

pub fn build(cfg: Config) {
    let frame = Frame::builder()
        .with_title("Pubseed")
        .with_size(Size::new(900, 700))
        .build();
    let root = Panel::builder(&frame).build();
    let root_sizer = BoxSizer::builder(Orientation::Vertical).build();

    let notebook = Notebook::builder(&root).build();
    let tab_panels: [Panel; 4] = std::array::from_fn(|_| Panel::builder(&notebook).build());
    let tab_sizers: [BoxSizer; 4] =
        std::array::from_fn(|_| BoxSizer::builder(Orientation::Vertical).build());
    for tab in 0..4 {
        tab_panels[tab].set_sizer(tab_sizers[tab], true);
        notebook.add_page(&tab_panels[tab], TAB_NAMES[tab], tab == 0, None);
    }
    root_sizer.add(&notebook, 1, SizerFlag::Expand, 0);

    // The player, along the bottom.
    let bar = Panel::builder(&root).build();
    let bar_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    let play_button = Button::builder(&bar).with_label("Play").build();
    let position_label = StaticText::builder(&bar).with_label("Position").build();
    let position = Slider::builder(&bar)
        .with_min_value(0)
        .with_max_value(1000)
        .build();
    crate::acc::name(&position, "Position");
    let volume_label = StaticText::builder(&bar).with_label("Volume").build();
    let volume = Slider::builder(&bar)
        .with_min_value(0)
        .with_max_value(100)
        .with_value(cfg.volume as i32)
        .build();
    crate::acc::name(&volume, "Volume");
    let centred = SizerFlag::All | SizerFlag::AlignCenterVertical;
    bar_sizer.add(&play_button, 0, centred, 4);
    bar_sizer.add(&position_label, 0, centred, 4);
    bar_sizer.add(&position, 1, centred, 4);
    bar_sizer.add(&volume_label, 0, centred, 4);
    bar_sizer.add(&volume, 0, centred, 4);
    bar.set_sizer(bar_sizer, true);
    root_sizer.add(&bar, 0, SizerFlag::Expand, 0);
    bar.hide();
    root.set_sizer(root_sizer, true);

    let status = frame.create_status_bar(3, 0, ID_ANY as Id, "statusBar");
    status.set_status_widths(&[-3, -2, -2]);
    frame.set_menu_bar(build_menu_bar(cfg.read_chat, cfg.speed, cfg.skip_seconds));
    speech::install(frame.get_handle());

    let app = Rc::new(App {
        api: Arc::new(Api::new(&cfg.site_url, &cfg.stream_url, cfg.token())),
        player: RefCell::new(Player::new(cfg.volume, cfg.speed)),
        cfg: RefCell::new(cfg),
        user: RefCell::new(None),
        now: RefCell::new(None),
        queue: RefCell::new((Vec::new(), 0)),
        room: RefCell::new(None),
        composer: RefCell::new(None),
        tabs: std::array::from_fn(|_| RefCell::new(Vec::new())),
        w: Widgets {
            frame,
            notebook,
            tab_panels,
            tab_sizers,
            root,
            player_bar: bar,
            play_button,
            position,
            volume,
            status,
        },
        next_page_id: Cell::new(0),
        modal: Cell::new(0),
        ticks: Cell::new(0),
        timer: RefCell::new(None),
    });
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    app.set_user(None);

    // A menu choice, from the menu bar or a row's menu. `call_after` lets the
    // menu finish closing before a command opens a dialog or closes a page.
    frame.on_menu_selected(move |event| {
        let id = event.get_id();
        wxdragon::call_after(Box::new(move || {
            if let Some(app) = self::app() {
                dispatch(&app, id);
            }
        }));
    });
    frame.on_char_hook(move |event| {
        let used = match (&event, self::app()) {
            (WindowEventData::Keyboard(key), Some(app))
                if !key.control_down() && !key.alt_down() =>
            {
                handle_key(&app, key.get_key_code().unwrap_or(0), key.shift_down())
            }
            _ => false,
        };
        event.skip(!used);
    });
    notebook.on_page_changed(move |_| {
        if let Some(app) = self::app()
            && let Some(page) = app.current()
        {
            pages::ensure_loaded(&app, &page);
        }
    });
    play_button.on_click(move |_| {
        if let Some(app) = self::app() {
            playback::toggle(&app);
        }
    });
    position.on_slider(move |event| {
        if let Some(app) = self::app() {
            playback::seek_fraction(&app, event.get_value() as f64 / 1000.0);
        }
    });
    volume.on_slider(move |event| {
        if let Some(app) = self::app() {
            let percent = event.get_value().clamp(0, 100) as u32;
            app.cfg.borrow_mut().volume = percent;
            app.player.borrow().set_volume(percent);
        }
    });
    frame.on_close(move |event| {
        if let Some(app) = self::app() {
            if let Some(timer) = app.timer.borrow_mut().take() {
                timer.stop();
            }
            if let Some(room) = app.room.borrow().as_ref() {
                room.cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            app.player.borrow_mut().stop();
            app.cfg.borrow().save();
        }
        APP.with(|slot| *slot.borrow_mut() = None);
        event.skip(true);
    });

    let kinds = [
        Kind::Feed(Source::Home),
        Kind::Feed(Source::Subscriptions),
        Kind::Live,
        Kind::Notifications,
    ];
    for (tab, kind) in kinds.into_iter().enumerate() {
        pages::open(&app, tab, kind);
    }

    let timer = Timer::new(&frame);
    timer.on_tick(move |_| {
        if let Some(app) = self::app() {
            let ticks = app.ticks.get() + 1;
            app.ticks.set(ticks);
            playback::tick(&app);
            composer::tick(&app);
            // Every three minutes.
            if ticks % 720 == 0 {
                pages::check_feed(&app);
            }
            // Once a minute, and shortly after starting.
            if ticks % 240 == 8 {
                pages::poll_unread(&app);
            }
        }
    });
    timer.start(250, false);
    *app.timer.borrow_mut() = Some(timer);

    frame.show(true);
    frame.centre();
    if let Some(page) = app.top(TAB_HOME) {
        page.list.set_focus();
    }
}

/// Sets a tab's label, with a count after it when there is one.
pub fn set_tab_count(app: &App, tab: usize, count: i64) {
    let label = if count > 0 {
        format!("{} ({count})", TAB_NAMES[tab])
    } else {
        TAB_NAMES[tab].to_string()
    };
    app.w.notebook.set_page_text(tab, &label);
}
