/* adviverse.h — C ABI for showing Adviverse ads in a pure-native desktop app
 * (Qt, Cocoa/AppKit, WinUI/WPF, GTK). Link against libadviverse (static or
 * dynamic). See README.md for Qt/Cocoa/WinUI usage.
 */
#ifndef ADVIVERSE_H
#define ADVIVERSE_H

#ifdef __cplusplus
extern "C" {
#endif

/* An ad. Any char* field may be NULL. `ttl` is seconds the ad may be shown.
 * Owned by the library — free with adviverse_free_ad(). */
typedef struct AdviverseAd {
  char* text;            /* headline (always present on a fill) */
  char* body;            /* optional secondary text */
  char* cta;             /* optional call to action, e.g. "Try free" */
  char* brand;           /* optional advertiser/brand */
  char* url;             /* tracked click URL — pass to adviverse_open_click */
  char* short_url;       /* compact fallback link */
  char* impression_url;  /* pass to adviverse_fire_impression when visible */
  char* label;           /* disclosure marker, e.g. "ad" — always show it */
  int   ttl;
} AdviverseAd;

/* Fetch an ad. BLOCKING — call OFF the UI thread. Returns NULL on no-fill /
 * offline / error. Free the result with adviverse_free_ad().
 *   token        : "ts_..." serve token | "ak_..." API key | publisher tag  (required)
 *   uid          : stable per-install id (NULL → a default)
 *   gdpr_consent : IAB TCF consent string, or NULL
 *   us_privacy   : US-Privacy string, or NULL
 *   client       : app label for reporting (NULL → "desktop")
 *   base         : serve host override (NULL → https://serve.adviverse.com)
 *   timeout_ms   : <= 0 → 2500ms default
 */
AdviverseAd* adviverse_fetch_ad(const char* token,
                                const char* uid,
                                const char* gdpr_consent,
                                const char* us_privacy,
                                const char* client,
                                const char* base,
                                int timeout_ms);

/* Free an ad from adviverse_fetch_ad(). Safe on NULL. */
void adviverse_free_ad(AdviverseAd* ad);

/* Fire the impression beacon ONCE, when the ad is actually visible to the user.
 * Non-blocking. Safe on NULL. */
void adviverse_fire_impression(const char* impression_url);

/* Open the landing URL in the user's DEFAULT browser (http/https only). */
void adviverse_open_click(const char* url);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* ADVIVERSE_H */
