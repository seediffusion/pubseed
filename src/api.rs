//! AudioPub client. The site has no dedicated API, so this reads the same
//! SvelteKit endpoints the web pages use: `__data.json` for page data, form
//! actions for changes, and server-sent events for live chat. The session is
//! the `token` cookie set by `POST /login`.

use crate::devalue;
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT, COOKIE, ORIGIN, SET_COOKIE};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

pub type Result<T> = std::result::Result<T, String>;

pub const SIGN_IN_FIRST: &str = "Sign in to do this. Use File, Sign in.";

/// Label, `sort` and `order` query values for the home feed.
pub const SORTS: [(&str, &str, &str); 6] = [
    ("Newest first", "createdAt", "DESC"),
    ("Oldest first", "createdAt", "ASC"),
    ("Most played", "plays", "DESC"),
    ("Most favorited", "favoriteCount", "DESC"),
    ("Title", "title", "ASC"),
    ("Random", "random", "DESC"),
];

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct User {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub bio: String,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Audio {
    pub id: String,
    pub title: String,
    pub description: String,
    pub plays: i64,
    pub favorite_count: i64,
    pub is_favorited: Option<bool>,
    pub created_at: i64,
    pub user: Option<User>,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Stream {
    pub id: String,
    pub title: String,
    pub description: String,
    pub state: String,
    pub active_listeners: i64,
    pub created_at: i64,
    pub user: Option<User>,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Chat {
    pub id: String,
    pub content: String,
    pub created_at: i64,
    pub user: User,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Comment {
    pub id: String,
    pub content: String,
    pub created_at: i64,
    pub user: User,
    pub replies: Option<Vec<Comment>>,
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Notification {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub target_type: String,
    /// An audio, stream or comment, depending on `target_type`.
    pub target: Value,
    pub metadata: Value,
    pub actor: Option<User>,
    pub read_at: Option<i64>,
    pub created_at: i64,
}

/// Which list of audios a page shows.
#[derive(Clone, Debug)]
pub enum Source {
    Home,
    Subscriptions,
    Favorites,
    Search(String),
    User(String),
}

#[derive(Default)]
pub struct Listing {
    pub streams: Vec<Stream>,
    pub audios: Vec<Audio>,
    pub has_more: bool,
    /// The signed-in user, as the layout reports it with every page.
    pub user: Option<User>,
    /// Set for `Source::User` only.
    pub profile: Option<Profile>,
}

pub struct Profile {
    pub user: User,
    pub subscribers: i64,
    pub is_subscribed: bool,
    pub stream: Option<Stream>,
}

pub struct AudioPage {
    pub audio: Audio,
    pub comments: Vec<Comment>,
    pub is_subscribed: bool,
    /// Whether the signed-in user is told about new comments on it.
    pub is_following: bool,
    /// Whether the signed-in user may edit or delete this audio.
    pub can_edit: bool,
    /// How many edits they have left; `None` for no limit.
    pub remaining_edits: Option<i64>,
}

pub struct StreamPage {
    pub stream: Stream,
    pub chats: Vec<Chat>,
}

pub enum LiveEvent {
    Chat(Chat),
    ChatDeleted(String),
    Listeners(i64),
    Finished,
}

pub struct Api {
    http: Client,
    /// No overall timeout: live audio and the chat feed never end.
    streaming: Client,
    pub base: String,
    pub stream_base: String,
    token: Mutex<Option<String>>,
}

fn take<T: DeserializeOwned + Default>(data: &mut Map<String, Value>, key: &str) -> T {
    data.remove(key)
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn net(e: reqwest::Error) -> String {
    format!("Network error: {e}")
}

impl Api {
    pub fn new(site_url: &str, stream_url: &str, token: Option<String>) -> Self {
        let agent = concat!("pubseed/", env!("CARGO_PKG_VERSION"));
        let builder = || {
            Client::builder()
                .user_agent(agent)
                .connect_timeout(Duration::from_secs(10))
        };
        Self {
            http: builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
            streaming: builder().timeout(None).build().expect("HTTP client"),
            base: site_url.trim_end_matches('/').to_string(),
            stream_base: stream_url.trim_end_matches('/').to_string(),
            token: Mutex::new(token),
        }
    }

    pub fn token(&self) -> Option<String> {
        self.token.lock().unwrap().clone()
    }

    pub fn sign_out(&self) {
        *self.token.lock().unwrap() = None;
    }

    fn with_session(&self, request: RequestBuilder) -> RequestBuilder {
        match self.token() {
            Some(token) => request.header(COOKIE, format!("token={token}")),
            None => request,
        }
    }

    /// Loads a page's data. A redirect comes back as `{"__redirect": location}`.
    fn data(&self, path: &str, query: &[(&str, String)]) -> Result<Map<String, Value>> {
        let url = format!("{}{}/__data.json", self.base, path.trim_end_matches('/'));
        let response = self
            .with_session(self.http.get(url).query(query))
            .send()
            .map_err(net)?;
        let status = response.status();
        let body: Value = response
            .json()
            .map_err(|_| format!("The server answered {status}"))?;
        if body["type"] == "redirect" {
            let location = body["location"].as_str().unwrap_or_default().to_string();
            if location.starts_with("/login") {
                return Err(SIGN_IN_FIRST.into());
            }
            return Ok(Map::from_iter([(
                "__redirect".to_string(),
                Value::String(location),
            )]));
        }
        let error = body["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|node| node["type"] == "error");
        if let Some(node) = error {
            return Err(node["error"]["message"]
                .as_str()
                .unwrap_or("The server reported an error")
                .to_string());
        }
        if !status.is_success() {
            return Err(format!("The server answered {status}"));
        }
        Ok(devalue::page_data(&body))
    }

    /// Runs a SvelteKit form action, such as `/listen/{id}?/favorite`.
    fn action(&self, path: &str, name: &str, form: &[(&str, &str)]) -> Result<Response> {
        let suffix = if name.is_empty() {
            String::new()
        } else {
            format!("?/{name}")
        };
        let request = self
            .http
            .post(format!("{}{path}{suffix}", self.base))
            // Asks for the JSON envelope instead of a browser redirect.
            .header(ACCEPT, "application/json")
            // SvelteKit rejects cross-site form posts by comparing this.
            .header(ORIGIN, &self.base)
            .form(form);
        self.with_session(request).send().map_err(net)
    }

    fn action_ok(&self, path: &str, name: &str, form: &[(&str, &str)]) -> Result<()> {
        if self.token().is_none() {
            return Err(SIGN_IN_FIRST.into());
        }
        let response = self.action(path, name, form)?;
        envelope(response).map(|_| ())
    }

    /// Signs in, returning the session token to remember and who signed in.
    pub fn login(&self, email: &str, password: &str) -> Result<(String, Option<User>)> {
        let response = self.action("/login", "", &[("email", email), ("password", password)])?;
        let token = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|cookie| cookie.strip_prefix("token="))
            .map(|rest| rest.split(';').next().unwrap_or_default().to_string());
        envelope(response)?;
        let token = token.ok_or("The server did not start a session")?;
        *self.token.lock().unwrap() = Some(token.clone());
        // Every page's data says who is signed in.
        let user = take(&mut self.data("/favorites", &[])?, "user");
        Ok((token, user))
    }

    pub fn listing(&self, source: &Source, sort: usize, page: u32) -> Result<Listing> {
        let mut query = vec![("page", page.to_string())];
        let path = match source {
            Source::Home => {
                let (_, field, order) = SORTS[sort.min(SORTS.len() - 1)];
                query.push(("sort", field.to_string()));
                query.push(("order", order.to_string()));
                "/".to_string()
            }
            Source::Subscriptions => "/subscriptions".to_string(),
            Source::Favorites => "/favorites".to_string(),
            Source::Search(text) => {
                query.push(("q", text.clone()));
                "/search".to_string()
            }
            Source::User(id) => format!("/user/{id}"),
        };
        let mut data = self.data(&path, &query)?;
        if data.contains_key("__redirect") {
            return Err(match source {
                Source::User(_) => "That user was not found".to_string(),
                _ => SIGN_IN_FIRST.to_string(),
            });
        }
        let mut audios: Vec<Audio> = take(&mut data, "audios");
        let has_more = match data.get("totalPages").and_then(Value::as_u64) {
            Some(total) => u64::from(page) < total,
            // Search reports no page count; a full page means there may be more.
            None => audios.len() >= 30,
        };
        let profile = matches!(source, Source::User(_)).then(|| Profile {
            user: take(&mut data, "profileUser"),
            subscribers: take(&mut data, "subscribers"),
            is_subscribed: take(&mut data, "isSubscribed"),
            stream: take(&mut data, "stream"),
        });
        // A profile's uploads come without their uploader, who is the profile.
        if let Some(profile) = &profile {
            for audio in &mut audios {
                audio.user.get_or_insert_with(|| profile.user.clone());
            }
        }
        Ok(Listing {
            streams: if profile.is_some() {
                Vec::new()
            } else {
                take(&mut data, "streams")
            },
            audios,
            has_more,
            user: take(&mut data, "user"),
            profile,
        })
    }

    pub fn audio(&self, id: &str) -> Result<AudioPage> {
        let mut data = self.data(&format!("/listen/{id}"), &[])?;
        if data.contains_key("__redirect") {
            return Err("This is a live stream, not a recording. Find it on the Live tab.".into());
        }
        Ok(AudioPage {
            audio: take(&mut data, "audio"),
            comments: take(&mut data, "comments"),
            is_subscribed: take(&mut data, "isSubscribed"),
            is_following: take(&mut data, "isFollowing"),
            can_edit: take(&mut data, "canEdit"),
            remaining_edits: take(&mut data, "remainingEdits"),
        })
    }

    pub fn stream(&self, id: &str) -> Result<StreamPage> {
        let mut data = self.data(&format!("/live/{id}"), &[])?;
        if data.contains_key("__redirect") {
            return Err("That stream has ended".into());
        }
        Ok(StreamPage {
            stream: take(&mut data, "stream"),
            chats: take(&mut data, "chats"),
        })
    }

    pub fn notifications(&self) -> Result<Vec<Notification>> {
        if self.token().is_none() {
            return Err("Sign in to see your notifications. Use File, Sign in.".into());
        }
        Ok(take(
            &mut self.data("/notifications", &[])?,
            "notifications",
        ))
    }

    pub fn unread_notifications(&self) -> Result<i64> {
        let request = self.http.get(format!("{}/notifications", self.base));
        let body: Value = self
            .with_session(request)
            .send()
            .map_err(net)?
            .json()
            .map_err(net)?;
        Ok(body["unread"].as_i64().unwrap_or(0))
    }

    pub fn set_favorite(&self, audio_id: &str, favorite: bool) -> Result<()> {
        let name = if favorite { "favorite" } else { "unfavorite" };
        self.action_ok(&format!("/listen/{audio_id}"), name, &[])
    }

    /// Turns notifications about new comments on an audio on or off.
    pub fn set_following(&self, audio_id: &str, follow: bool) -> Result<()> {
        let name = if follow { "follow" } else { "unfollow" };
        self.action_ok(&format!("/listen/{audio_id}"), name, &[])
    }

    /// Subscribes to, or unsubscribes from, the person `path` is about: an
    /// audio's uploader (`/listen/{id}`) or a profile (`/user/{id}`).
    pub fn set_subscribed(&self, path: &str, subscribe: bool) -> Result<()> {
        let name = if subscribe {
            "subscribe"
        } else {
            "unsubscribe"
        };
        self.action_ok(path, name, &[])
    }

    pub fn add_comment(&self, audio_id: &str, text: &str, parent: Option<&str>) -> Result<()> {
        let mut form = vec![("comment", text)];
        if let Some(parent) = parent {
            form.push(("parentId", parent));
        }
        self.action_ok(&format!("/listen/{audio_id}"), "add_comment", &form)
    }

    pub fn delete_comment(&self, audio_id: &str, comment_id: &str) -> Result<()> {
        self.action_ok(
            &format!("/listen/{audio_id}"),
            "delete_comment",
            &[("id", comment_id)],
        )
    }

    pub fn delete_notification(&self, id: &str) -> Result<()> {
        self.action_ok("/notifications", "delete", &[("id", id)])
    }

    /// Deletes every notification.
    pub fn clear_notifications(&self) -> Result<()> {
        self.action_ok("/notifications", "clear_all", &[])
    }

    /// Uploads a new audio and returns its id. `sent` counts the bytes of
    /// the file sent so far.
    pub fn upload(
        &self,
        file: &Path,
        title: &str,
        description: &str,
        sent: Arc<AtomicU64>,
    ) -> Result<String> {
        if self.token().is_none() {
            return Err(SIGN_IN_FIRST.into());
        }
        let reader =
            std::fs::File::open(file).map_err(|e| format!("Couldn't open the audio: {e}"))?;
        let length = reader
            .metadata()
            .map_err(|e| format!("Couldn't read the audio: {e}"))?
            .len();
        // The site takes the file's extension from its name.
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio");
        let part = Part::reader_with_length(
            Counting {
                inner: reader,
                sent,
            },
            length,
        )
        .file_name(name.to_string());
        let form = Form::new()
            .text("title", title.to_string())
            .text("description", description.to_string())
            .part("file", part);
        // No overall timeout: a large upload takes as long as it takes.
        let request = self
            .streaming
            .post(format!("{}/upload", self.base))
            .header(ACCEPT, "application/json")
            .header(ORIGIN, &self.base)
            .multipart(form);
        let location = envelope(self.with_session(request).send().map_err(net)?)?;
        match location.strip_prefix("/listen/") {
            Some(id) if !id.is_empty() => Ok(id.to_string()),
            _ => Err("The upload went through, but the site didn't say where it is".into()),
        }
    }

    /// Changes an audio's title and description.
    pub fn edit_audio(&self, audio_id: &str, title: &str, description: &str) -> Result<()> {
        let form = [("title", title), ("description", description)];
        self.action_ok(&format!("/listen/{audio_id}"), "edit", &form)
    }

    pub fn delete_audio(&self, audio_id: &str) -> Result<()> {
        self.action_ok(&format!("/listen/{audio_id}"), "delete", &[])
    }

    /// Counts a play, as the web player does when playback starts.
    pub fn register_play(&self, audio_id: &str) {
        let url = format!("{}/listen/{audio_id}/try_register_play", self.base);
        let _ = self.http.post(url).send();
    }

    pub fn send_chat(&self, stream_id: &str, content: &str) -> Result<()> {
        if self.token().is_none() {
            return Err(SIGN_IN_FIRST.into());
        }
        let request = self
            .http
            .post(format!("{}/live/{stream_id}", self.base))
            .json(&serde_json::json!({ "content": content }));
        let response = self.with_session(request).send().map_err(net)?;
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        let body: Value = response.json().unwrap_or_default();
        Err(body["message"]
            .as_str()
            .map(str::to_string)
            .unwrap_or(format!("The server answered {status}")))
    }

    /// Opens an audio file or live stream for playback.
    pub fn open_audio(&self, url: &str) -> Result<Response> {
        let response = self.streaming.get(url).send().map_err(net)?;
        if !response.status().is_success() {
            return Err(format!("The server answered {}", response.status()));
        }
        Ok(response)
    }

    /// Follows a stream's live events until it finishes or `cancel` is set,
    /// reconnecting if the feed drops. Blocks; run it on its own thread.
    pub fn follow_events(
        &self,
        stream_id: &str,
        cancel: &AtomicBool,
        mut on_event: impl FnMut(LiveEvent),
    ) {
        let url = format!("{}/live/{stream_id}/events", self.base);
        while !cancel.load(Ordering::Relaxed) {
            let request = self.streaming.get(&url).header(ACCEPT, "text/event-stream");
            if let Ok(response) = self.with_session(request).send() {
                // A finished stream answers 204 rather than an event feed.
                if response.status() == reqwest::StatusCode::NO_CONTENT {
                    on_event(LiveEvent::Finished);
                    return;
                }
                let (mut name, mut payload) = (String::new(), String::new());
                for line in BufReader::new(response).lines() {
                    let Ok(line) = line else { break };
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Some(value) = line.strip_prefix("event:") {
                        name = value.trim().to_string();
                    } else if let Some(value) = line.strip_prefix("data:") {
                        payload = value.trim().to_string();
                    } else if line.is_empty() && !name.is_empty() {
                        let data: Value = serde_json::from_str(&payload).unwrap_or_default();
                        let event = match name.as_str() {
                            "chat" => serde_json::from_value(data).ok().map(LiveEvent::Chat),
                            "chat_delete" => data["chatId"]
                                .as_str()
                                .map(|id| LiveEvent::ChatDeleted(id.to_string())),
                            "listeners" => {
                                data["activeListeners"].as_i64().map(LiveEvent::Listeners)
                            }
                            "finish" => Some(LiveEvent::Finished),
                            _ => None,
                        };
                        let finished = matches!(event, Some(LiveEvent::Finished));
                        if let Some(event) = event {
                            on_event(event);
                        }
                        if finished {
                            return;
                        }
                        name.clear();
                        payload.clear();
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

/// Passes a file through, counting the bytes read from it.
struct Counting<R> {
    inner: R,
    sent: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.sent.fetch_add(read as u64, Ordering::Relaxed);
        Ok(read)
    }
}

/// Reads a form action's JSON envelope: `{"type": "success" | "redirect" |
/// "failure" | "error", ...}`. The HTTP status alone is not trustworthy.
/// Returns where a redirect points, or an empty string.
fn envelope(response: Response) -> Result<String> {
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|_| format!("The server answered {status}"))?;
    match body["type"].as_str() {
        Some("success") | Some("redirect") => {
            Ok(body["location"].as_str().unwrap_or_default().to_string())
        }
        Some("failure") => {
            // `data` is itself a devalue-encoded string.
            let message = body["data"]
                .as_str()
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .map(|data| devalue::page_data(&serde_json::json!({ "nodes": [{ "data": data }] })))
                .and_then(|data| {
                    data.get("message")
                        .or(data.get("editMessage"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            Err(message.unwrap_or_else(|| "The server refused that".to_string()))
        }
        _ => Err(body["error"]["message"]
            .as_str()
            .or(body["message"].as_str())
            .map(str::to_string)
            .unwrap_or(format!("The server answered {status}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the real site's public home feed.
    /// Run with `cargo test live_site -- --ignored`.
    #[test]
    #[ignore]
    fn live_site_home_feed_parses() {
        let api = Api::new("https://audiopub.site", "https://live.audiopub.site", None);
        let listing = api.listing(&Source::Home, 0, 1).unwrap();
        assert!(!listing.audios.is_empty());
        assert!(!listing.audios[0].id.is_empty());
        assert!(listing.audios[0].user.is_some());
        let page = api.audio(&listing.audios[0].id).unwrap();
        assert_eq!(page.audio.id, listing.audios[0].id);
        assert!(api.notifications().is_err());
        assert_eq!(
            api.set_favorite(&page.audio.id, true).unwrap_err(),
            SIGN_IN_FIRST
        );
        let refused = api.login("nobody@example.invalid", "wrong").unwrap_err();
        assert_eq!(refused, "Invalid email or password");
    }
}
