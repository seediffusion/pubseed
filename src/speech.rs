//! Speech: through the running screen reader, or through an installed voice.
//!
//! The screen reader is reached with UI Automation notification events raised
//! from the main window (the approach Pubsplash uses), which NVDA, JAWS and
//! Narrator all speak without any extra DLLs. Voices are SAPI voices.

use std::cell::RefCell;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Media::Speech::{
    ISpObjectToken, ISpObjectTokenCategory, ISpVoice, SPF_ASYNC, SPF_IS_NOT_XML,
    SPF_PURGEBEFORESPEAK, SpObjectTokenCategory, SpVoice,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    IRawElementProviderSimple, IRawElementProviderSimple_Impl, NotificationKind_Other,
    NotificationProcessing_All, NotificationProcessing_ImportantMostRecent, ProviderOptions,
    ProviderOptions_ServerSideProvider, UIA_ControlTypePropertyId, UIA_NamePropertyId,
    UIA_PATTERN_ID, UIA_PROPERTY_ID, UIA_WindowControlTypeId, UiaHostProviderFromHwnd,
    UiaRaiseNotificationEvent, UiaReturnRawElementProvider, UiaRootObjectId,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::WM_GETOBJECT;
use windows::core::{BSTR, Error, IUnknown, PCWSTR, Result, implement};

const VOICE_CATEGORIES: [&str; 2] = [
    r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Speech\Voices",
    r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Speech_OneCore\Voices",
];

thread_local! {
    static PROVIDER: RefCell<Option<IRawElementProviderSimple>> = const { RefCell::new(None) };
    static VOICE: RefCell<Option<ISpVoice>> = const { RefCell::new(None) };
    /// The name of the voice `VOICE` was last set to; empty for the default.
    static CURRENT_VOICE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A UI Automation element for the main window that exists only to be the
/// source of notification events.
#[implement(IRawElementProviderSimple)]
struct FrameProvider {
    hwnd: isize,
}

impl IRawElementProviderSimple_Impl for FrameProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, _pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        Err(Error::empty())
    }

    fn GetPropertyValue(&self, property: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(if property == UIA_ControlTypePropertyId {
            VARIANT::from(UIA_WindowControlTypeId.0)
        } else if property == UIA_NamePropertyId {
            VARIANT::from("Pubseed")
        } else {
            VARIANT::default()
        })
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        // The default provider keeps the window's controls visible.
        unsafe { UiaHostProviderFromHwnd(HWND(self.hwnd as *mut _)) }
    }
}

unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_GETOBJECT
        && lparam.0 as i32 == UiaRootObjectId
        && let Some(provider) = PROVIDER.with(|p| p.borrow().clone())
    {
        return unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &provider) };
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Makes the main window able to speak through the screen reader.
pub fn install(frame_hwnd: *mut std::ffi::c_void) {
    let provider: IRawElementProviderSimple = FrameProvider {
        hwnd: frame_hwnd as isize,
    }
    .into();
    PROVIDER.with(|p| *p.borrow_mut() = Some(provider));
    unsafe {
        let _ = SetWindowSubclass(HWND(frame_hwnd), Some(frame_proc), 0x5EEE, 0);
    }
}

/// Speaks through the screen reader. `interrupt` replaces anything Pubseed
/// said before; otherwise messages queue up, as chat needs.
pub fn announce(text: &str, interrupt: bool) {
    PROVIDER.with(|p| {
        if let Some(provider) = p.borrow().as_ref() {
            let processing = if interrupt {
                NotificationProcessing_ImportantMostRecent
            } else {
                NotificationProcessing_All
            };
            unsafe {
                let _ = UiaRaiseNotificationEvent(
                    provider,
                    NotificationKind_Other,
                    processing,
                    &BSTR::from(text),
                    &BSTR::from("pubseed"),
                );
            }
        }
    });
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn voice_tokens() -> Vec<(String, ISpObjectToken)> {
    let mut found = Vec::new();
    for category_id in VOICE_CATEGORIES {
        unsafe {
            let Ok(category) = CoCreateInstance::<_, ISpObjectTokenCategory>(
                &SpObjectTokenCategory,
                None,
                CLSCTX_ALL,
            ) else {
                continue;
            };
            let id = wide(category_id);
            if category.SetId(PCWSTR(id.as_ptr()), false).is_err() {
                continue;
            }
            let Ok(tokens) = category.EnumTokens(PCWSTR::null(), PCWSTR::null()) else {
                continue;
            };
            let mut count = 0u32;
            let _ = tokens.GetCount(&mut count);
            for index in 0..count {
                let Ok(token) = tokens.Item(index) else {
                    continue;
                };
                // The default value of a voice token is its display name.
                if let Ok(description) = token.GetStringValue(PCWSTR::null()) {
                    let name = description.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(description.as_ptr() as *const _));
                    if !found.iter().any(|(existing, _)| *existing == name) {
                        found.push((name, token));
                    }
                }
            }
        }
    }
    found
}

/// The names of the installed voices.
pub fn voices() -> Vec<String> {
    voice_tokens().into_iter().map(|(name, _)| name).collect()
}

/// Speaks with an installed voice (`name` empty for the default), queued
/// behind anything already being said unless `interrupt` is set.
pub fn say(name: &str, rate: i32, text: &str, interrupt: bool) {
    VOICE.with(|slot| {
        let mut slot = slot.borrow_mut();
        // Looking a voice up means listing them all, so only on a change.
        let changed = CURRENT_VOICE.with(|current| current.replace(name.to_string()) != name);
        // A fresh voice object is the way back to the Windows default.
        if slot.is_none() || (changed && name.is_empty()) {
            *slot = unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL).ok() };
        }
        let Some(voice) = slot.as_ref() else { return };
        unsafe {
            if changed
                && let Some((_, token)) =
                    voice_tokens().into_iter().find(|(found, _)| found == name)
            {
                let _ = voice.SetVoice(&token);
            }
            let _ = voice.SetRate(rate.clamp(-10, 10));
            let mut flags = SPF_ASYNC.0 | SPF_IS_NOT_XML.0;
            if interrupt {
                flags |= SPF_PURGEBEFORESPEAK.0;
            }
            let text = wide(text);
            let _ = voice.Speak(PCWSTR(text.as_ptr()), flags as u32, None);
        }
    });
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    #[test]
    fn installed_voices_are_listed() {
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let voices = super::voices();
        assert!(!voices.is_empty());
        assert!(voices.iter().all(|name| !name.is_empty()));
    }
}
