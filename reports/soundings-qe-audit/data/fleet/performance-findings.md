# Soundings Regions-Map — Performance Findings

**Analyst:** QE fleet performance agent · **Date:** 2026-07-13
**Target:** https://soundings-23l.pages.dev/regions-map (read-only curl probes; no browser driven)
**Source of measured metrics:** `../evidence-brief.md` (GPU browser sidecar, Chrome).

---

## Current-vs-achievable (one line)

**First load on 4G (~10 Mbps): ~25 s today → ~2 s achievable (>10×); repeat visit ~4 s of pointless 304 round-trips → near-instant.** The 21.7 MB / 19 s cold start is a *design* cost, not a network limit: the default view force-loads every region's full point set, and the payload is 52 MB of decompressed **JavaScript source** the engine must parse.

## Content-encoding actually found

**`content-encoding: br` — brotli IS already active** on the HTML and on every `data/*_map_data.js` file (Cloudflare automatic). But it is CF's *dynamic, low-quality* brotli: on London the brotli body is **7,006,084 B — 4.8 % LARGER than gzip's 6,683,382 B** (identity 20,255,945 B). Brotli losing to gzip is the signature of CF compressing on-the-fly at ~quality 4–5. Static max-brotli (q11) precompression at build time typically beats gzip by 15–20 % — so there is still a brotli win on the table (London ~7.0 → ~5.5 MB) even though brotli is "on".

---

## 1. Root-cause analysis of the 19 s load

### 1a. The default view is the whole-UK point cloud, and it force-loads every region

The code comment claims on-demand loading (lines 227–229):

```js
// from data/regions_index.js and loaded on demand — adding a swept region
// needs no change to this page.
```

That is true for *clicking a region leaf* — but **not for the boot path**. `init()` defaults to the UK aggregate (lines 970–973):

```js
document.getElementById("subtitle").textContent =
  "Assembling everything gathered so far…";
payload = await loadUK();
id = "uk";
```

`loadUK()` merges **every** NAV entry (line 616):

```js
const loadUK = () => loadSuper(NAV, { id: "uk", name: "United Kingdom",
  center: [54.30, -3.00], zoom: 5.9, clusterZoom: 8.0 });
```

`loadSuper` walks the whole hierarchy and fetches each region's *full* dataset — **serially at the top level** (lines 601–608):

```js
for (const item of items){
  try {
    const payload = await loadWholeOf(item);   // <-- awaited one NAV entry at a time
    if (payload) parts.push({ label: item.name, payload });
  } catch (e){ console.warn(meta.name, "view: skipping", item.name, e); }
}
```

…and each `combine:true` group pulls all its children (line 579):

```js
const payloads = await Promise.all(kids.map(([, s])=>loadRegion(s)));
```

So the default map is **745,485 individual company points**, assembled client-side from ~38 data files on the critical path. Traced payload (my sum below matches the brief's England = 689,793 exactly, confirming the path):

| Cluster | Companies | % of pts | Est. br |
|---|--:|--:|--:|
| **London** (1 file) | 289,520 | 38.6 % | ~7.0 MB |
| South East (7 files) | 105,529 | 14.1 % | ~2.6 MB |
| East of England (5) | 67,946 | 9.1 % | ~1.7 MB |
| North West (5) | 57,595 | 7.7 % | ~1.4 MB |
| South West (8) | 47,138 | 6.3 % | ~1.2 MB |
| West Midlands (6) | 44,134 | 5.9 % | ~1.1 MB |
| Yorkshire & Humber | 33,772 | 4.5 % | ~0.8 MB |
| East Midlands | 31,514 | 4.2 % | ~0.8 MB |
| Scotland | 29,838 | 4.0 % | ~0.8 MB |
| Wales | 21,297 | 2.8 % | ~0.5 MB |
| North East | 12,645 | 1.7 % | ~0.3 MB |
| Northern Ireland | 8,324 | 1.1 % | ~0.2 MB |
| **Critical-path total (dedup → 745,485)** | | | **~18.6 MB** |

**London alone is 38.8 % of the points and ~32 % of the bytes.**

### 1b. An idle prefetch loop then pulls the *remaining* ~29 files

After boot, `init()` queues **every** region — including the aggregate/sub-region overlaps `loadUK` didn't need — and trickle-loads + parses all of them (lines 1016–1024):

```js
const queue = INDEX.map(r=>r.slug);
const prefetchNext = ()=>{
  const slug = queue.shift();
  if (!slug) return;
  (loadedData[slug] ? Promise.resolve(loadedData[slug]) : loadRegion(slug))
    .then(p=>warmParse(p)).catch(()=>{})
    .finally(()=>idle(prefetchNext, { timeout: 4000 }));
};
idle(prefetchNext, { timeout: 10000 });
```

This is why the *total* transfer is 21.7 MB (67 files) even though the UK view only needs ~18.6 MB (38 files) — the extra ~3 MB is aggregate+sub-region double-coverage prefetched "just in case".

### 1c. Why 19 s on a *fast* link — it is parse + merge, not bandwidth

On the GPU-sidecar link, 21.7 MB should arrive in ~2–3 s, yet the load event is **19.0 s**. The dominant cost is client-side:

- **Parsing ~52 MB of JavaScript source.** Each file is a giant array *literal* (`GLOBAL = [["Aardman…",51.4,-2.6,[1,3],4,2,"01426907",1976], …]`). The JS engine must lex/parse every one of 745k rows of source — seconds of main-thread work, not a typed-array view.
- **`mergePayloads()`** concatenates and dedupes 745k rows on company number (lines 549–553), and **`warmParse()`** maps all 450k+ rows into objects with `titleCase()` string work (lines 421–438). The comment concedes it: *"Parsing 450k rows for the UK view takes hundreds of ms"* (×several passes: UK merge, warmParse, idle re-warm).

### 1d. GPU stall — the UK set never leaves the GPU

`compose()` keeps the full UK layer set mounted (invisible) even after you drill into a region, so its buffers stay resident (lines 1159–1161):

```js
// The UK set stays mounted (invisible) so returning to it is free.
if (S.region !== "uk" && parsedCache.uk)
  L.push(...layerSetFor("uk", parsedCache.uk, false, _ukIdleOp));
```

745k points' position/colour/radius attributes sit in device-local buffers and are re-mapped on every filter/mode transition → the repeated *"GPU stall due to mapping device local buffer"* warning.

### 1e. What the UK view actually needs

The per-region **counts already exist** in `regions_index.js` and are the *only* thing the tree labels consume (line 653, and `updateNav` lines 840–844):

```js
const countOf = slug => (INDEX.find(r=>r.slug===slug) || {}).companies;
```

The 21.7 MB of point data is needed **only to draw dots** — and at UK zoom (5.9) 745k dots overplot into an undifferentiated density cloud where no individual point is legible. So the aggregate view needs (a) totals → already free from the 1.6 KB index, and (b) a *low-resolution density representation*, not 745k full-fidelity rows. Full rows are only meaningful once the user drills into a region — which the existing `switchRegion`/`loadRegion` lazy path already handles perfectly.

---

## 2. Prioritised optimisation plan

### (a) Lazy default view + index-driven totals — **biggest win**
- Do **not** call `loadUK()` at boot. Default to a pre-built, spatially-binned **overview payload** (aggregate 745k → ~50k representative/quantised points, or a quadkey density grid). Totals come from `regions_index.js` (already loaded, 1.6 KB br).
- Keep the existing per-region lazy path for drill-down (already implemented and correct).
- Delete or gate the "prefetch every region" loop (1b) behind an explicit user hint (e.g. only prefetch the region under the cursor / on hover).
- **Impact:** initial transfer **21.7 MB → ~1.4 MB (~15×)**; removes the 52 MB parse and the client-side UK merge from the critical path. This single change does ~90 % of the work.

### (b) Binary columnar format (typed arrays) instead of JS source
- Replace the `GLOBAL = [[...], ...]` array-literal files with an `ArrayBuffer` (fetched, not `<script>`-injected), laid out **struct-of-arrays**: `Float32`/quantised-`Int32` lat & lng columns, `Uint8` sizeTier/boroughIdx, `Int16` year, `Uint32` Companies-House number, and a single concatenated **name blob** with a `Uint32` offset index. (deck.gl consumes typed-array accessors natively — `getPosition` can read a Float32Array directly.)
- **Wire size:** columnar numeric data brotli-compresses better than row-wise source punctuation → ~**15–20 % smaller** (national ~18.6 → ~15 MB *if* fully loaded; the overview payload is ~1 MB).
- **Parse time:** `new Float32Array(buffer)` is ~0 ms vs seconds of JS parsing → this is what removes the 19 s on *any* link, fast or slow.
- **Impact:** parse ~seconds → ~100 ms; wire −15–20 %; eliminates main-thread jank during load.

### (c) Immutable caching + fingerprinted filenames
- Current headers: `cache-control: public, max-age=0, must-revalidate` + ETag. Every repeat visit issues a **conditional GET per file**: measured **304, 0-byte body, but ~108 ms TTFB each** → ~38 critical files ≈ **~4 s of pure round-trips that transfer nothing**.
- Fingerprint names (`greater_london.<hash>.bin`) and serve `cache-control: public, max-age=31536000, immutable` via a Cloudflare Pages `_headers` file. Repeat visits: **zero** data-file requests; only HTML + index revalidate.
- **Impact:** repeat-visit data cost → ~0; removes ~4 s of revalidation latency.

### (d) Brotli — already on, but upgrade to static max-brotli
- **Confirmed active** (`content-encoding: br`) but at CF's low dynamic quality — brotli (7.006 MB) is *larger* than gzip (6.683 MB) on London, proving q≈4. Ship build-time **q11** precompressed assets (CF Pages serves a matching precompressed variant).
- **Impact:** ~15–20 % on top of what's there (London ~7.0 → ~5.5 MB) for the region files that *do* download on drill-down. Free once (a)/(b) shrink what's loaded.

### (e) Viewport-based deck.gl windowing (GPU stall)
- Stop keeping the full UK set mounted after drill-in (1d): unmount it, or gate with deck.gl `DataFilterExtension` / a quadkey tile scheme so only viewport-visible points hold GPU buffers.
- Pair with (a): the low-res overview means the zoomed-out layer is ~50k points, not 745k, so no device-buffer thrash at the default zoom.
- **Impact:** removes the "GPU stall … mapping device local buffer" warnings; smoother filter/mode transitions.

**Priority order:** (a) ≫ (b) > (c) ≈ (e) > (d).

---

## 3. Estimated end-state — 4G (~10 Mbps = 1.25 MB/s) arithmetic

**Current (first load):**
- Critical-path bytes that must arrive before the UK map renders: **18.6 MB** → 18.6 / 1.25 = **14.9 s network**.
- Plus ~5 s parsing 52 MB of decompressed JS + client-side merge (top-level region loads are `await`-serial, so network and parse only partially overlap).
- Plus deck.gl (470 KB, one-time, long-cached) + Google Maps bootstrap ~1 s.
- **Realistic 4G time-to-interactive ≈ 22–28 s (~25 s).** (The 19 s measured on the fast sidecar link was parse/merge-bound; 4G adds ~13 s of network the fast link didn't pay.)

**Achievable (first load), with (a)+(b)+(c)+(d):**
- HTML ~14 KB br + `regions_index.js` 1.6 KB br + deck.gl 470 KB (cached after first ever visit) + **overview binary ~1.0 MB** ≈ **1.4 MB** → 1.4 / 1.25 = **1.1 s network**.
- Binary parse (typed-array views) ~0.1 s + Maps init ~0.6 s.
- **≈ 2 s to an interactive UK overview.**
- Region drill-down (London, worst case): ~5.0 MB (max-brotli + columnar) → **4 s**, user-initiated with a spinner, then **immutably cached** (0 s on any later visit).

**Repeat visit:** today ≈ ~4 s of 304 round-trips (no data, pure latency); with (c) ≈ **~0.3 s** (HTML + index only).

| Scenario | Today | Achievable |
|---|--:|--:|
| First load, 4G | ~25 s | **~2 s** |
| First load, fast link | 19 s | **~1.5 s** |
| Drill into London | (already loaded) | ~4 s, then cached |
| Repeat visit | ~4 s revalidation | **~0.3 s** |
| Initial transfer | 21.7 MB | **~1.4 MB** |
| JS parsed on load | ~52 MB | **~1 MB binary** |

---

## 4. What is already good (keep these)

- **Compact tuple rows** — `[name, lat, lng, [sectorIdx…], sizeTier, boroughIdx, chNo, year]` with an integer **sector dictionary** per region (`payload.sectors`). Already dictionary-encoded and free of per-field JSON keys; the remaining win is *binary*, not schema.
- **Static hosting on Cloudflare Pages + CDN edge** (h3, CF-NEL) — low TTFB (~108 ms), global POPs, and brotli-on by default. Right platform.
- **Single persistent deck.gl layer set** — mode/filter changes re-colour the *same* dots via GPU attribute transitions, and view switches crossfade by **opacity** reusing untouched buffers (lines 1178–1180). Genuinely efficient rendering; only the "keep UK mounted forever" part (1d) needs bounding.
- **Client-side caching discipline** — `parsedCache`, memoised `visibleCompanies()`, `warmParse()` in `requestIdleCallback` slots, and `useDevicePixels` capped at 1.5. Good instincts; the fix is to stop doing this work for *all* regions eagerly, not to remove the caches.
- **Graceful degradation** — `gm_authFailure` + MutationObserver quota handling; regions mid-export render greyed "soon" and drop out of merges rather than breaking the view.

---

## Appendix — probe commands (read-only)

```
# brotli confirmed, and brotli LARGER than gzip on London (low CF quality):
curl -s -o /dev/null -w '%{size_download}' -H 'accept-encoding: br'   .../greater_london_map_data.js   # 7,006,084
curl -s -o /dev/null -w '%{size_download}' -H 'accept-encoding: gzip' .../greater_london_map_data.js   # 6,683,382
curl -s -o /dev/null -w '%{size_download}' -H 'accept-encoding: identity' .../greater_london_map_data.js # 20,255,945

# repeat-visit revalidation: 304, 0-byte body, ~108 ms TTFB per file
curl -s -o /dev/null -w '%{http_code} %{time_starttransfer}s' -H 'If-None-Match: W/"…"' .../greater_london_map_data.js  # 304 0.108s

# headers: cache-control: public, max-age=0, must-revalidate  (no immutable, no fingerprint)
```

~25 br bytes/company (London 24.2, Scotland 25.2, Yorkshire 25.9); ~70 raw bytes/company.
