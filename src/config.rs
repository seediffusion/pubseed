//! Settings, saved as JSON in `%APPDATA%\pubseed\settings.json`. The
//! sign-in is kept apart from them, in Windows Credential Manager.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use windows::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};
use windows::core::{PCWSTR, PWSTR};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
pub enum SpeechOutput {
    #[default]
    ScreenReader,
    Voice,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub site_url: String,
    /// Base of the Icecast listen URLs; a stream is at `{stream_url}/{user id}`.
    pub stream_url: String,
    pub appearance: Appearance,
    pub volume: u32,
    pub skip_seconds: u32,
    pub sort: u32,
    pub read_chat: bool,
    pub speech_output: SpeechOutput,
    /// SAPI voice name; empty for the default voice.
    pub voice: String,
    pub voice_rate: i32,
    /// Playback speed; 1.0 is normal.
    pub speed: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            site_url: "https://audiopub.site".into(),
            stream_url: "https://live.audiopub.site".into(),
            appearance: Appearance::System,
            volume: 100,
            skip_seconds: 10,
            sort: 0,
            read_chat: true,
            speech_output: SpeechOutput::ScreenReader,
            voice: String::new(),
            voice_rate: 0,
            speed: 1.0,
        }
    }
}

fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("pubseed")
        .join("settings.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    /// The saved session for this site, if someone is signed in.
    pub fn token(&self) -> Option<String> {
        read_credential(&credential_name(&self.site_url))
    }

    /// Saves the session for this site, or forgets it with `None`.
    pub fn set_token(&self, token: Option<&str>) {
        let name = credential_name(&self.site_url);
        match token {
            Some(token) => write_credential(&name, token),
            None => delete_credential(&name),
        }
    }
}

/// The session is the site's `token` cookie. It goes in Windows Credential
/// Manager, never in the settings file; the password itself is never saved
/// anywhere.
fn credential_name(site_url: &str) -> String {
    format!("Pubseed: {}", site_url.trim_end_matches('/'))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn read_credential(name: &str) -> Option<String> {
    let name = wide(name);
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(
            PCWSTR(name.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
        .ok()?;
        let blob = std::slice::from_raw_parts(
            (*credential).CredentialBlob,
            (*credential).CredentialBlobSize as usize,
        );
        let token = String::from_utf8(blob.to_vec()).ok();
        CredFree(credential as *const _);
        token.filter(|token| !token.is_empty())
    }
}

fn write_credential(name: &str, secret: &str) {
    let mut name = wide(name);
    let mut user = wide("session");
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(name.as_mut_ptr()),
        UserName: PWSTR(user.as_mut_ptr()),
        CredentialBlobSize: secret.len() as u32,
        CredentialBlob: secret.as_ptr() as *mut u8,
        // Kept on this computer for this Windows user; it does not roam.
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    unsafe {
        let _ = CredWriteW(&credential, 0);
    }
}

fn delete_credential(name: &str) {
    let name = wide(name);
    unsafe {
        let _ = CredDeleteW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_kept_in_credential_manager_not_the_settings_file() {
        let config = Config {
            site_url: "https://pubseed-test.invalid".into(),
            ..Config::default()
        };
        config.set_token(Some("secret-jwt"));
        assert_eq!(config.token().as_deref(), Some("secret-jwt"));
        assert!(!serde_json::to_string(&config).unwrap().contains("secret"));
        config.set_token(None);
        assert_eq!(config.token(), None);
    }
}
