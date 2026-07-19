//! adviverse-desktop-native — a small C ABI for showing Adviverse ads in a
//! PURE-NATIVE desktop app (Qt, Cocoa/AppKit, WinUI/WPF, GTK) that has no
//! webview. Same model as the terminal/mobile SDKs: fetch a JSON ad, render it
//! in your own widgets, fire the impression when it's visible, open clicks in
//! the default browser.
//!
//! Serving routes by token prefix: `ts_` (public serve token) and `ak_` (API
//! key) hit the self-network serve endpoint; anything else is a publisher tag.
//!
//! All FFI entry points are blocking on the network EXCEPT the impression (which
//! is fired on a background thread). Call `adviverse_fetch_ad` OFF your UI thread.

#![allow(clippy::missing_safety_doc)]

use serde::Deserialize;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::time::Duration;

#[derive(Deserialize, Default)]
struct AdJson {
    text: Option<String>,
    body: Option<String>,
    cta: Option<String>,
    brand: Option<String>,
    url: Option<String>,
    short_url: Option<String>,
    impression_url: Option<String>,
    label: Option<String>,
    ttl: Option<i32>,
}

/// An ad, C-owned. Any `*char` field may be NULL. `ttl` is seconds. Free with
/// `adviverse_free_ad`.
#[repr(C)]
pub struct AdviverseAd {
    pub text: *mut c_char,
    pub body: *mut c_char,
    pub cta: *mut c_char,
    pub brand: *mut c_char,
    pub url: *mut c_char,
    pub short_url: *mut c_char,
    pub impression_url: *mut c_char,
    pub label: *mut c_char,
    pub ttl: c_int,
}

fn cstr(s: Option<String>) -> *mut c_char {
    match s.filter(|v| !v.is_empty()) {
        Some(v) => CString::new(v).map(|c| c.into_raw()).unwrap_or(ptr::null_mut()),
        None => ptr::null_mut(),
    }
}

unsafe fn opt(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p)
        .to_str()
        .ok()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Fetch an ad. BLOCKING — call off the UI thread. Returns a heap `AdviverseAd*`
/// (free with `adviverse_free_ad`) or NULL on no-fill / offline / error. Pass
/// NULL for any optional arg. `timeout_ms <= 0` uses the 2500ms default.
#[no_mangle]
pub extern "C" fn adviverse_fetch_ad(
    token: *const c_char,
    uid: *const c_char,
    gdpr_consent: *const c_char,
    us_privacy: *const c_char,
    client: *const c_char,
    base: *const c_char,
    timeout_ms: c_int,
) -> *mut AdviverseAd {
    let (token, uid, gdpr, usp, client, base) = unsafe {
        (
            match opt(token) {
                Some(t) => t,
                None => return ptr::null_mut(),
            },
            opt(uid).unwrap_or_else(|| "ins_native".to_string()),
            opt(gdpr_consent),
            opt(us_privacy),
            opt(client).unwrap_or_else(|| "desktop".to_string()),
            opt(base).unwrap_or_else(|| "https://serve.adviverse.com".to_string()),
        )
    };

    let timeout = Duration::from_millis(if timeout_ms > 0 { timeout_ms as u64 } else { 2500 });
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();

    let (path, api_key) = if token.starts_with("ts_") {
        (format!("{}/terminal/serve", base), None)
    } else if token.starts_with("ak_") {
        (format!("{}/terminal/serve", base), Some(token.clone()))
    } else {
        (format!("{}/terminal", base), None)
    };

    let mut req = agent
        .get(&path)
        .query("uid", &uid)
        .query("client", &client);
    if token.starts_with("ts_") {
        req = req.query("token", &token);
    } else if !token.starts_with("ak_") {
        req = req.query("tag", &token);
    }
    if let Some(g) = &gdpr {
        req = req.query("gdpr_consent", g);
    }
    if let Some(u) = &usp {
        req = req.query("us_privacy", u);
    }
    if let Some(k) = &api_key {
        req = req.set("x-api-key", k);
    }

    let body = match req.call() {
        Ok(r) => r.into_string().unwrap_or_default(),
        Err(_) => return ptr::null_mut(),
    };
    let ad: AdJson = match serde_json::from_str(&body) {
        Ok(a) => a,
        Err(_) => return ptr::null_mut(),
    };
    let text = match ad.text {
        Some(t) if !t.is_empty() => t,
        _ => return ptr::null_mut(), // no-fill
    };

    Box::into_raw(Box::new(AdviverseAd {
        text: cstr(Some(text)),
        body: cstr(ad.body),
        cta: cstr(ad.cta),
        brand: cstr(ad.brand),
        url: cstr(ad.url),
        short_url: cstr(ad.short_url),
        impression_url: cstr(ad.impression_url),
        label: cstr(Some(ad.label.unwrap_or_else(|| "ad".to_string()))),
        ttl: ad.ttl.unwrap_or(60),
    }))
}

/// Free an ad returned by `adviverse_fetch_ad`. Safe on NULL.
#[no_mangle]
pub extern "C" fn adviverse_free_ad(ad: *mut AdviverseAd) {
    if ad.is_null() {
        return;
    }
    unsafe {
        let a = Box::from_raw(ad);
        for p in [
            a.text, a.body, a.cta, a.brand, a.url, a.short_url, a.impression_url, a.label,
        ] {
            if !p.is_null() {
                drop(CString::from_raw(p));
            }
        }
    }
}

/// Fire the impression beacon (GET) once, when the ad is actually on-screen.
/// Non-blocking (runs on a background thread). Safe on NULL.
#[no_mangle]
pub extern "C" fn adviverse_fire_impression(impression_url: *const c_char) {
    let url = unsafe { opt(impression_url) };
    if let Some(u) = url {
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(5))
                .build();
            let _ = agent.get(&u).call();
        });
    }
}

/// Open the ad's landing URL in the user's DEFAULT browser. http(s) only.
#[no_mangle]
pub extern "C" fn adviverse_open_click(url: *const c_char) {
    let url = unsafe { opt(url) };
    if let Some(u) = url {
        if u.starts_with("http://") || u.starts_with("https://") {
            let _ = open::that(u);
        }
    }
}
