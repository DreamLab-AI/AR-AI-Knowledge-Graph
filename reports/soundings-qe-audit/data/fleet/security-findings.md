# Soundings Regions-Map — Security Findings

**Target:** https://soundings-23l.pages.dev/regions-map (Cloudflare Pages, public static data product)
**Assessed:** 2026-07-13 — read-only header/endpoint probes + full source review (`data/regions-map.html`, 1,429 lines)
**Assessor:** QE Security Auditor (fleet)
**Method:** Static source analysis (SAST-style DOM sink tracing) + read-only `curl` probes of headers, the Google Maps key, and the unpkg CDN. No browser driven, no mutating/flooding scanner run.

---

## Overall risk rating: **MEDIUM**

Rated MEDIUM **solely** because of one confirmed, anyone-can-reproduce financial exposure: the embedded Google Maps API key is **not effectively restricted** and serves billable Static Maps + Geocoding requests from a non-browser context. Every other issue is low-severity hardening. There is **no data-confidentiality risk** (all data is the public Companies House register, no PII, no auth, no cookies, no backend), and — notably — **the XSS surface is well-defended**: every data-derived value that reaches `innerHTML` is passed through a correct `esc()` escaper. Absent the billing exposure this would be a LOW-risk site.

### Finding counts by severity
| Severity | Count | Findings |
|---|---|---|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 3 | Unrestricted billable Maps key; no CSP; deck.gl supply-chain (mutable tag, no SRI) |
| Low | 4 | No HSTS; no X-Frame-Options/frame-ancestors; no Permissions-Policy; password-type key input |
| Info / positive | 4 | XSS correctly escaped (positive); ACAO `*` benign for open data; no PII beyond public register; `?key=` localStorage poisoning (self-healing) |

---

## API-key restriction probe — RESULT: **key is NOT effectively restricted (billable, quota-theft exposed)**

The source comment (lines 869–870) asserts *"Site key: referrer-restricted in Google Cloud Console, so it only works on this site's domains."* **The probes refute this.**

| Probe | Referer | Result | Meaning |
|---|---|---|---|
| Static Maps API (`/staticmap`) | none | **HTTP 200**, valid `PNG 400×400`, 52,584 bytes | Real map tile served — key works with no referer |
| Static Maps API | `https://evil.example.com/` | **HTTP 200**, 16,924-byte PNG | Referer is not gated |
| Geocoding API (`/geocode/json`) | none | **HTTP 200**, `"status":"OK"`, real Liverpool/Merseyside results | Web-service API enabled + unrestricted |
| Geocoding API | `https://evil.example.com/` | **HTTP 200**, `"status":"OK"` | Hostile referer accepted |

**Conclusion:** the key `AIzaSyCYBppM6M1sUQa3MMjzGQHkAIijXLOPEmI` is not IP-restricted, and any HTTP-referrer restriction present is not enforced for these endpoints. Two independent web-service APIs (Static Maps, Geocoding) return billable results to an anonymous, non-browser caller with an arbitrary referer.

**Why HTTP-referrer restriction alone can never fix this:** referrer restrictions only apply to *browser* surfaces (Maps JavaScript API, Static Maps, Street View Static). **Web-service APIs — Geocoding, Directions, Places, etc. — cannot be referrer-restricted at all**; a browser key exposing them can only be protected by *not enabling them on this key* or by IP-restriction (which is impossible for a browser key). The Geocoding success proves the key has web-service APIs enabled with no effective restriction.

**Quota-theft / denial-of-wallet risk (either way):** the key is in plain page source (line 871). Google Maps Platform bills per request beyond the monthly credit. An attacker can scrape the key and script Geocoding/Static Maps/Directions calls billed to the owner's Google Cloud account — exhausting the credit and then accruing real charges. Even *with* a working referrer restriction the Maps-JS quota is still spendable by spoofing the `Referer` header from a script (referrer restriction is a soft control, not authentication). This is the single reason the site is rated MEDIUM rather than LOW.

---

## Findings (severity-ordered)

### [MEDIUM] SEC-1 — Google Maps API key is unrestricted and billable (denial-of-wallet)
**CWE-799 (improper control of interaction frequency) / cost-abuse.** Evidence above. The key is embedded (line 871) and additionally accepted via `?key=` and localStorage.

**Remediation (do all three in Google Cloud Console → Credentials → this API key):**
1. **Application restriction → HTTP referrers**, allow-list exactly: `https://soundings-23l.pages.dev/*` (and any custom domain). This protects the Maps JS + Static Maps surfaces from casual reuse.
2. **API restriction → restrict to "Maps JavaScript API" only.** Explicitly **disable Geocoding, Static Maps, Directions, Places** on this key. The app only needs the Maps JavaScript API (vector basemap). Disabling the web-service APIs closes the un-restrictable exfil path the probe exploited. Re-run the two Geocoding probes after — both must then return `REQUEST_DENIED`.
3. **Set a billing budget + alert** (e.g. cap at the free credit) and, if available, a **quota cap** on Maps JS map loads per day. This bounds the worst case to "map temporarily unavailable" (which the app already degrades to gracefully via `gm_authFailure` / the MutationObserver on `.gm-err-container`) instead of an unbounded bill.
> Note: the key cannot be "hidden" — any browser-loaded Maps key is public by design. The correct posture is restriction + budget caps, not secrecy.

### [MEDIUM] SEC-2 — No Content-Security-Policy
**CWE-693 (protection mechanism failure).** No CSP header (confirmed absent). XSS is otherwise well-mitigated (see SEC-8), but a CSP is the highest-leverage header here because it also constrains where the third-party deck.gl bundle (SEC-3) and any future injected script could **load code from and exfiltrate to** — directly reducing the blast radius of a supply-chain compromise and of the exposed key.

**Recommended CSP for this exact app.** The clean fix is to first **externalise the single inline `<script>` block** (lines 222–1426) into a same-origin `app.js`; the code uses only DOM-property handlers (`el.onclick = …`, `addEventListener`) — **no inline HTML `on*=` attributes** — so removing `'unsafe-inline'` from `script-src` will not break any handler. Set via a Cloudflare Pages `_headers` file:

```
/*
  Content-Security-Policy: default-src 'self'; script-src 'self' https://unpkg.com https://maps.googleapis.com https://maps.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; img-src 'self' data: blob: https://*.googleapis.com https://*.gstatic.com; connect-src 'self' https://maps.googleapis.com https://*.googleapis.com https://*.gstatic.com; worker-src blob:; font-src 'self' https://fonts.gstatic.com; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'; upgrade-insecure-requests
```

Notes on why each source is required:
- `script-src`: `'self'` (data files + the externalised app.js), `unpkg.com` (deck.gl), `maps.googleapis.com`/`maps.gstatic.com` (Maps JS + the loader-injected script, whose `nonce` propagation at line 921 becomes unnecessary once hosts are allow-listed).
- `style-src 'unsafe-inline'`: **mandatory** — Google Maps injects inline styles at runtime and cannot run without it; this is a known, accepted Google Maps constraint. It does not weaken script protection.
- `img-src`/`connect-src` `*.googleapis.com`+`*.gstatic.com data: blob:`: vector tiles, sprites, and metadata XHR.
- `worker-src blob:`: deck.gl and Maps GL spawn blob workers.
- `frame-ancestors 'none'` also delivers the clickjacking protection of SEC-5.

**Interim (if you cannot externalise the inline script yet):** add `'unsafe-inline'` to `script-src`. This yields a *weaker* XSS control but still enforces `object-src 'none'`, `base-uri 'self'`, `frame-ancestors 'none'`, and — most importantly here — a `connect-src`/`script-src` **host allow-list that blocks exfiltration/loading to arbitrary domains**. Alternatively pin an inline-script `'sha384-…'` hash (stable per deploy, static-host friendly).

### [MEDIUM] SEC-3 — Supply chain: deck.gl from unpkg via mutable `@9` tag, no SRI
**CWE-829 (inclusion of functionality from untrusted control sphere).** Line 8: `<script src="https://unpkg.com/deck.gl@9/dist.min.js">`. Probe: `@9` **302-redirects to `deck.gl@9.3.6`** — a mutable major-version range that silently resolves to whatever the latest 9.x is at load time. The bundle runs with **full page privileges**, including read access to the embedded Maps key. A compromised unpkg, a hijacked deck.gl npm publish token, or a malicious 9.x release would execute a Magecart-style payload on every visit with **zero integrity checking** (SRI is impossible on a moving tag).

**Remediation (ranked):**
1. **Best — self-host a vendored, pinned copy.** Download `deck.gl@9.3.6/dist.min.js` (1,646,672 bytes) to `/vendor/deck.gl-9.3.6.min.js`, reference it same-origin (covered by `script-src 'self'`), and serve it `cache-control: public, max-age=31536000, immutable`. Removes third-party trust and the unpkg availability dependency entirely.
2. **Good — pin the exact version on unpkg + add SRI.** Pinning to an immutable exact version makes the SRI hash stable:
   ```html
   <script src="https://unpkg.com/deck.gl@9.3.6/dist.min.js"
           integrity="sha384-UX+cI12FBGambH8nU4vd3AUqzXpQ/NHM4mLI4wbBRYT9AtLRJjKgzYQQeGShiAdR"
           crossorigin="anonymous"></script>
   ```
   (SRI hash computed from the live 9.3.6 bundle during this audit.) Re-pin + re-hash deliberately on each upgrade.

### [LOW] SEC-4 — No Strict-Transport-Security (HSTS)
**CWE-319.** Header absent. Mitigating factors: HTTP already 301-redirects to HTTPS (probe confirmed), the site is on Cloudflare's `*.pages.dev` (TLS-only infrastructure), and there are **no cookies, no auth, no state** to strip. Residual risk is a first-visit SSL-strip MITM on a hostile network — low impact for a read-only public dataset. Add anyway (trivial, best practice):
```
Strict-Transport-Security: max-age=31536000; includeSubDomains
```
(Add `; preload` only if you intend to submit the custom domain to the preload list.)

### [LOW] SEC-5 — No X-Frame-Options / CSP frame-ancestors
**CWE-1021 (UI redress / clickjacking).** Absent. The app is a read-only map with no state-changing action, so clickjacking impact is minimal; the only conceivable target is tricking a user into typing their *own* Maps key into the gate overlay — contrived and low-value. Fixed for free by `frame-ancestors 'none'` in the SEC-2 CSP; optionally also send `X-Frame-Options: DENY` for legacy UAs.

### [LOW] SEC-6 — No Permissions-Policy
**CWE-693 / hardening.** Absent. The app requests no geolocation, camera, microphone, or payment features, so send a restrictive policy to lock those down defensively:
```
Permissions-Policy: geolocation=(), camera=(), microphone=(), payment=(), usb=(), interest-cohort=()
```

### [LOW] SEC-7 — API-key input is `type="password"` (browser credential-manager confusion)
**CWE-522-adjacent / UX-security smell (not a secret leak).** Line 159: `<input id="gkey" type="password" …>`. A Maps API key is not a login credential; `type="password"` makes Chrome emit the "password field is not contained in a form" warning and invites the browser password manager to offer to save / autofill the user's key into their vault. No secret is exposed (the field only ever holds a *user-supplied* key that already lives in that user's own `localStorage`), and this path is rarely reached because normal visitors auto-boot on the embedded key (line 874). Fix:
```html
<input id="gkey" type="text" inputmode="text" autocomplete="off"
       autocapitalize="off" spellcheck="false" placeholder="Google Maps API key">
```

### [INFO / POSITIVE] SEC-8 — XSS surface is correctly defended
Company names originate from the Companies House register and can legally contain `<`, `>`, `&`. I traced every DOM sink: **all data-derived values reaching `innerHTML` are escaped** by a correct escaper:
```js
function esc(s){ return String(s ?? "").replace(/[&<>"]/g, c=>({ "&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;" }[c])); }
```
- Search results (line 1264) — `esc(c.name)`, `esc(c.primary)` ✓
- Detail card `<h3>` / District / badges (lines 1330–1338) — `esc(c.name)`, `esc(c.d)`, `esc(g)` ✓
- Companies House deep link (line 1335) — `href="…/company/${esc(c.num)}"` in a **double-quoted** attribute; `esc()` escapes `"`, so no attribute breakout, and the `https://…` scheme is fixed (no `javascript:` injection). Also carries `rel="noopener"` ✓
- deck.gl tooltip (lines 1148–1149) — `esc(object.name)`; the unescaped `groups`/`TIER_LABEL` fragments are from hardcoded constants, not raw data ✓
- Region tree, chips, crumbs — `esc()` on all labels/names ✓
- Numeric-only sinks (`counts`, chip counts) use `toLocaleString()`; titles/subtitles/cluster buttons use `textContent` ✓

**Minor defence-in-depth note (not a vulnerability):** `esc()` does not escape the single quote `'`. This is currently safe because no data value is ever placed inside a *single-quoted* HTML attribute. If a future edit introduces `style='…${value}…'` or `attr='…'` with data, it would become exploitable — add `"'":"&#39;"` (and `` "`" ``) to the map now as a cheap guardrail. **No action required for correctness today.**

### [INFO / POSITIVE] SEC-9 — Wildcard ACAO is benign-to-beneficial here
`access-control-allow-origin: *` on the page and data files (probe confirmed). For a public, read-only, **cookieless, credential-less** open-data product this is harmless and actively useful — it lets third-party tools consume the open dataset, and `ACAO: *` cannot be paired with credentials, so no cross-origin data-theft vector exists. No change recommended.

### [INFO] SEC-10 — Data/privacy: no PII beyond the public register
The per-company tuple is `[name, lat, lng, [sectorIdx…], sizeTier, boroughIdx, companiesHouseNo, yearIncorporated]` — all sourced from the Companies House bulk register (public, Open Government Licence). Positions are **registered-office postcode centroids, not exact addresses** (explicit in the footer and code comments), and there are **no directors' names, no residential addresses, no email/phone**. This is a deliberately privacy-preserving representation of already-public statutory data. No PII finding.

### [INFO] SEC-11 — `?key=` URL parameter writes to localStorage (self-healing annoyance)
Lines 872–873 persist any `?key=` value to `localStorage`. A crafted `?key=<garbage>` link could break a victim's map on that origin until cleared — but `init()` calls `localStorage.removeItem("gmaps_key")` on load failure (line 923), so it self-heals on the next reload. No data risk, no persistence of attacker content in the DOM. Optional hardening: ignore `?key=` unless the value matches Google's key format (`/^AIza[0-9A-Za-z_\-]{35}$/`).

---

## Positive controls observed
- `x-content-type-options: nosniff` present.
- `referrer-policy: strict-origin-when-cross-origin` present.
- HTTP → HTTPS 301 redirect enforced.
- Consistent, correct output escaping on all data-derived DOM sinks (SEC-8).
- `rel="noopener"` on the external Companies House link (anti reverse-tabnabbing).
- Graceful key-failure degradation (no leaking of Google error internals to users).
- No backend, no auth, no cookies, no CSRF surface — minimal attack surface by construction.

## Remediation priority
1. **SEC-1** — restrict the Maps key (referrer + API-restrict to Maps JS only + billing/quota cap). *Highest impact, config-only, ~15 min.*
2. **SEC-3** — self-host or SRI-pin deck.gl. *Closes the one full-privilege third-party code path.*
3. **SEC-2** — add CSP via `_headers` (externalise inline script for the strong variant).
4. **SEC-4/5/6** — add HSTS, frame-ancestors/XFO, Permissions-Policy in the same `_headers` file.
5. **SEC-7** — `type="text" autocomplete="off"` on the key input.
