# adviverse-desktop-native

Adviverse ads for **pure-native desktop apps** (Qt, Cocoa/AppKit, WinUI/WPF, GTK)
— apps with no webview. It's a tiny C ABI: fetch a JSON ad, render it in your own
widgets, fire the impression when it's visible, and open clicks in the default
browser. Same serving + integrity model as the terminal/mobile SDKs.

If your app already hosts a webview (Electron, Tauri, CEF), use the higher-level
[`@adviverse/desktop`](../desktop) instead — it renders the card for you.

## Build

```sh
cargo build --release          # → target/release/libadviverse.{a,dylib,so} (+ .dll on Windows)
```

Link `libadviverse` (static `.a` or dynamic) and include
[`include/adviverse.h`](include/adviverse.h). No async runtime, TLS via rustls.

## The loop

1. `adviverse_fetch_ad(...)` **off the UI thread** → `AdviverseAd*` (or NULL on no-fill).
2. Render `label` + `brand`/`text`/`body` + `cta` in your widget. **Always show `label`.**
3. When the widget is actually on-screen, call `adviverse_fire_impression(ad->impression_url)` once.
4. On click, `adviverse_open_click(ad->url)`.
5. `adviverse_free_ad(ad)` when done.

Get a `ts_…` serve token from your Adviverse dashboard. `ak_…` API keys and
publisher placement tags also work.

## Qt (C++)

```cpp
#include "adviverse.h"
#include <QtConcurrent>

void AdWidget::load() {
  QtConcurrent::run([this] {                              // off the UI thread
    AdviverseAd* ad = adviverse_fetch_ad("ts_yourtoken",
                                         nullptr, nullptr, nullptr, "qt-app", nullptr, 0);
    if (!ad) return;
    QString label = ad->label, text = ad->text;
    QString cta   = ad->cta ? ad->cta : "";
    QString url   = ad->url ? ad->url : "";
    QString imp   = ad->impression_url ? ad->impression_url : "";
    QMetaObject::invokeMethod(this, [=] {                 // back on the UI thread
      m_label->setText("[" + label + "] " + text + (cta.isEmpty() ? "" : "  " + cta + " →"));
      m_url = url;
      if (!imp.isEmpty()) adviverse_fire_impression(imp.toUtf8().constData());  // widget is visible
    });
    adviverse_free_ad(ad);
  });
}
void AdWidget::mousePressEvent(QMouseEvent*) {
  if (!m_url.isEmpty()) adviverse_open_click(m_url.toUtf8().constData());
}
```

## Cocoa / Swift (macOS)

Add a bridging header that includes `adviverse.h`, then:

```swift
DispatchQueue.global().async {                            // off the UI thread
  guard let ad = adviverse_fetch_ad("ts_yourtoken", nil, nil, nil, "mac-app", nil, 0) else { return }
  let text = String(cString: ad.pointee.text)
  let cta  = ad.pointee.cta.map { String(cString: $0) } ?? ""
  let url  = ad.pointee.url.map { String(cString: $0) }
  let imp  = ad.pointee.impression_url
  DispatchQueue.main.async {
    self.adLabel.stringValue = "[ad] \(text)" + (cta.isEmpty ? "" : "  \(cta) →")
    self.clickURL = url
    if let imp = imp { adviverse_fire_impression(imp) }   // view is on-screen
  }
  adviverse_free_ad(ad)
}
// on click:
if let u = clickURL { adviverse_open_click(u) }
```

## WinUI / WPF (C#, P/Invoke)

```csharp
static class Adviverse {
  const string L = "adviverse";                            // adviverse.dll
  [StructLayout(LayoutKind.Sequential)]
  public struct Ad { public IntPtr text, body, cta, brand, url, short_url, impression_url, label; public int ttl; }
  [DllImport(L)] public static extern IntPtr adviverse_fetch_ad(string token, string uid, string gdpr, string usp, string client, string @base, int timeoutMs);
  [DllImport(L)] public static extern void adviverse_free_ad(IntPtr ad);
  [DllImport(L)] public static extern void adviverse_fire_impression(string url);
  [DllImport(L)] public static extern void adviverse_open_click(string url);
  public static string? S(IntPtr p) => p == IntPtr.Zero ? null : Marshal.PtrToStringUTF8(p);
}

await Task.Run(() => {                                     // off the UI thread
  var p = Adviverse.adviverse_fetch_ad("ts_yourtoken", null, null, null, "winui-app", null, 0);
  if (p == IntPtr.Zero) return;
  var ad = Marshal.PtrToStructure<Adviverse.Ad>(p);
  var text = Adviverse.S(ad.text); var cta = Adviverse.S(ad.cta); var url = Adviverse.S(ad.url);
  var imp  = Adviverse.S(ad.impression_url);
  dispatcher.TryEnqueue(() => {
    AdText.Text = $"[ad] {text}" + (cta is null ? "" : $"  {cta} →");
    _clickUrl = url;
    if (imp is not null) Adviverse.adviverse_fire_impression(imp);   // control is visible
  });
  Adviverse.adviverse_free_ad(p);
});
// on click: if (_clickUrl is not null) Adviverse.adviverse_open_click(_clickUrl);
```

## Doing it right (the same bar as the web/mobile SDKs)

- **Viewability** — only call `adviverse_fire_impression` when the ad control is
  genuinely on-screen and the window is focused (not minimized/backgrounded).
  You own this signal in a native toolkit; wire it to your visibility events.
- **Disclosure** — always render `label` ("ad"). Store review (Mac App Store /
  Microsoft Store) requires ads be disclosed and non-deceptive.
- **Consent** — pass `gdpr_consent` / `us_privacy` when you have them.
- **Clicks** — `adviverse_open_click` opens the **system browser**; never
  navigate your app to the landing page.
- **Threading** — `adviverse_fetch_ad` blocks on the network; always call it off
  the UI thread (examples above do). The impression is already non-blocking.
