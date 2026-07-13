# Soundings `regions-map.html` — Code Review Findings

**Reviewer:** QE Code Reviewer (fleet code-quality analyst)
**Target:** `data/regions-map.html` — single-file vanilla-JS deck.gl app, 1,428 lines (64,332 bytes), ~1,205 lines inline JS + ~140 lines CSS + ~70 lines HTML.
**Date:** 2026-07-13
**Headline verdict:** Craft **7/10**. Genuinely skilled front-end work with a knowingly-shipped correctness inconsistency and zero test seams.

---

## 1. Architecture & craft

### What is genuinely good

- **No-framework discipline, done properly.** `"use strict"`, one script block, module-scoped `const`/`let` (not truly global — see below), clean separation into commented sections (data/constants → parsing → state → region loading → tree → boot → layers → render → panel → cards → helpers). This is disciplined vanilla JS, not jQuery-era spaghetti.
- **Compact tuple data format.** Companies ship as positional arrays `[name, lat, lng, [sectorIdx…], sizeTier, boroughIdx, companiesHouseNo, yearIncorporated]` (parsed at `warmParse`, lines 421–438) with sector names and district names factored out into per-file lookup tables. This is the correct bandwidth decision for 745k rows; a JSON-of-objects format would roughly triple transfer size. Sector indices are re-based onto a shared table on merge (`remap`, lines 543–546) so cross-region concatenation stays compact.
- **Sophisticated GPU-transition choreography.** One persistent set of scatterplot layers per view; mode/filter changes re-colour and re-size the *same* dots via deck.gl attribute transitions (`updateTriggers` keyed on `filterKey()`, lines 1095, 1108); view switches crossfade whole layer sets by *opacity* (`crossfade`, lines 1181–1200) with cloned "ghost" layers, avoiding a GPU buffer rebuild on the click frame. Filtered-out dots stay mounted at alpha 0 and shrink to 0.3× radius so they read as dissolving rather than cut (lines 1089–1092). This is real deck.gl expertise.
- **van Wijk & Nuij optimal zoom-pan flight path** (`flyTo`, lines 1349–1408). The camera flies along one hyperbolic arc (the maths behind MapLibre/Google Earth flights), hand-implemented in web-mercator world coordinates with a pure-zoom special case. ~60 lines of non-trivial, correct maths. This is a marker of a developer who reaches for the right algorithm rather than a naive lerp.
- **On-demand region discovery design.** `INDEX = NW_REGIONS.concat(SOUNDINGS_REGIONS)` (line 272); `navEntries()` (lines 678–693) renders the full configured hierarchy always, greys out leaves whose data hasn't landed (`leaf soon`), and sweeps any *un-named* exported region into an auto-generated "Other areas" group. `refreshIndex()` (lines 658–676) re-fetches the manifest every 60s and on every whole-region request, invalidating cached merges when new regions arrive. The design genuinely supports "regions land incrementally this evening, grow the tree live."
- **Graceful degradation.** `gm_authFailure` global + a `MutationObserver` watching for Google's `.gm-err-container` (lines 909–915) catch quota/billing/referrer failures and swap in a calm "map temporarily unavailable, resets 8am" panel (`showUnavailable`, lines 892–908) instead of Google's raw error dialog. Site-key failure is treated as an ops problem, not the visitor's (line 925). Thoughtful.
- **Honest provenance UI.** Footer and card both state positions are "registered-office postcode centroid — *claimed, not verified*", name the source (Companies House bulk register, July 2026), the licence (postcodes.io / OGL), and per-region generated dates. The "Fringe" district footnote (lines 200–201) openly admits postcode districts over-approximate at region edges. Intellectually honest for an evidence product.
- **Performance hygiene.** Memoised parse cache (`parsedCache`), memoised visibility (`_visCache`, lines 453–459), `requestIdleCallback` trickle-prefetch of every region so first clicks feel like revisits (lines 1014–1024), retina render cap (`useDevicePixels: min(dpr, 1.5)`, line 993) to stop laptop-GPU panning stutter, throttled zoom handler (line 997).

### Real weaknesses

- **Global namespace pollution via the data layer.** Each of the 67 `data/*_map_data.js` files assigns a `window` global (`window[entry.global]` / `window.REGION_MAPS[slug]`, line 473) plus `window.SOUNDINGS_REGIONS`. The *app* code is well-scoped, but the *data contract* is 60+ mutable window globals. `loadRegion` reads them by name and normalises (lines 469–479). Collisions are avoided only by naming convention.
- **Testability: zero.** No modules, no exports, no dependency injection, no seams. Every function closes over live singletons (`D`, `companies`, `S`, `map`, `overlay`) and most write straight to `document.getElementById(...)`. Pure logic that *could* be unit-tested — `titleCase`, `mergePayloads`, `warmParse`, the `updateNav` count reducer, `isVisible`, `filterKey` — is welded to module state and the DOM. There is no way to assert "England should equal 686,026" in a test without booting the whole page in a browser. For a product whose entire value proposition is *counts being correct*, this is the most consequential gap.
- **Unhandled rejection on failed data load.** `loadRegion` correctly rejects on `script.onerror` (line 481) and on missing payload (line 474). But `switchRegion` (lines 503–505) does `applyRegion(await loadRegion(slug), slug)` with no `.catch`. A single 404 or truncated data file on a leaf click produces an unhandled promise rejection and a dead click — no user-facing error. `loadSuper` *does* guard per-region (try/catch, lines 601–608) so whole-nation views degrade gracefully, but individual leaf navigation does not. The brief's finding 7 ("no error UI for failed data loads") is confirmed in code.
- **Magic numbers throughout.** `rho = 1.42` (flight arc), `13.2` (prime-label zoom threshold, appears 3×), `300`-iteration constants, `_fadeMs` defaults, `TIER_RADIUS` pixel values, `useDevicePixels 1.5`, `9` search-result cap, `60000` refresh interval. Mostly commented, but not named constants.
- **Dead / vestigial code.** `notifyGraphDataListeners`-style leftovers aside, `_ukIdleOp`/`_liveOp` state machine is intricate enough to be fragile; `boroughRow`'s `.on` toggling (line 765) and `switchLondonBorough`'s chip-matching by `dataset.count` string parsing (lines 761–764) is brittle string coupling. `groupOf` (lines 695–700) linear-scans NAV on every crumb rebuild.
- **Accessibility (from brief, confirmed in markup).** Tree rows, mode buttons, filter chips are `div`/`span` with `onclick` and no `role`/`tabindex` — mouse-only. Lighthouse's 94 understates this because the widgets are inert text to the auditor.
- **The eager-load contradiction.** The header comment (lines 225–229) claims non-NW regions are "loaded on demand"; `init()` then fires an idle prefetch loop over **every** region (lines 1014–1024) *plus* assembles the full UK view on boot (`loadUK`, line 972). Result: 21.7 MB / 19 s load. The lazy-load *machinery* exists and works; the boot path defeats it. This is a policy bug, not a capability gap — `init()` could prefetch nothing and the app would still function.

---

## 2. Correctness: root cause of the England count discrepancy

**Root cause (2–3 sentences):** The UK-level tree badge for England is computed in `updateNav()` at **lines 838–843** as a naive arithmetic sum of per-region index counts — mixing whole-region file counts for the slug-backed nations (North East, Yorkshire `yorkshire_humber`, East Midlands `east_midlands_region`=31,514, London) with sums of *child* counts for the five `combine` regions (North West, West Midlands, East of England, South East, South West). That sum dedups nothing: it double-counts child files that overlap within a combine region (most clearly `north_devon`=815 ⊂ `devon`=7,128 in the South West — a case the author's own comment flags at line 536) and companies whose registered-office postcode straddles a nation boundary (the "Humber unitaries in both the Yorkshire and East Midlands lenses", comment lines 614–615). The drill-down value (686,026) comes from `mergePayloads()` (lines 538–554), which **dedups on Companies House number** (`row[6]`, line 550: `if (seen.has(row[6])) continue`); the 3,767-company delta is exactly those overlaps that the naive sum keeps and the merge removes.

**Proof — reconstructing `updateNav` lines 838–843 against the shipped index reproduces 689,793 to the digit:**

| Nation | Tree value | How `updateNav` computes it |
|---|---:|---|
| North West | 57,595 | Σ kids (lcr+gm+cheshire+lancashire+cumbria) |
| North East | 12,645 | `countOf('north_east')` — whole file |
| Yorkshire & the Humber | 33,772 | `countOf('yorkshire_humber')` — whole file |
| East Midlands | 31,514 | `countOf('east_midlands_region')` — whole file |
| West Midlands | 44,134 | Σ kids |
| East of England | 67,946 | Σ kids |
| London | 289,520 | `countOf('greater_london')` — whole file |
| South East | 105,529 | Σ kids |
| South West | 47,138 | Σ kids (incl. `north_devon` **inside** `devon`) |
| **NAIVE TOTAL** | **689,793** | matches UK-tree England exactly |
| Drill (mergePayloads, deduped) | 686,026 | brief |
| **Delta** | **3,767** | double-counted overlaps |

The author *knew*: the comment at **lines 833–834** reads "Exact once the whole view has been assembled; until then the sum of the exported children (overlapping lenses may double-count)." The bug is that this "until then" estimate is shown as an authoritative badge with no visual disclaimer, so the UK tree and the England drill-down disagree by 3,767 in the same UI. Note the same mechanism drives the sub-region gaps in the brief: South East's tree value 105,529 is *exactly* the naive kids-sum above, versus its deduped drill of 105,283 (Δ 246); Yorkshire and East Midlands gaps are the same whole-file-vs-deduped-scope pattern, compounded by the "Fringe" postcode over-approximation the footnote already discloses.

**Trigger condition:** at the UK view, only `loadedData.uk` is assembled; `loadedData['all:England']` is not populated until the user actually clicks England (`switchEngland` → `loadEngland`, lines 618–628). So `merged = loadedData[groupId(item)]` (line 836) is `undefined`, `updateNav` falls to the naive-sum branch, and the badge shows 689,793. After drilling England, the cached deduped merge (686,026) is used everywhere. The two numbers can never agree until England is loaded.

**Fix (minimal):** compute the England (and every group) badge from the deduped whole-view rather than the index sum — i.e. eagerly (or lazily-on-first-nav-render) run the same company-number dedup used by `mergePayloads`, or suppress the numeric badge with a "~" prefix while `merged` is absent so the UI never asserts a precise wrong number. The cleanest structural fix is to precompute a global `Set` of Companies House numbers per nation once and size-badge from set cardinality, eliminating the sum path entirely.

---

## 3. Maintainability — does "add a region = drop a data file" hold?

**Mostly, with one caveat.** Verified from code:
- A new `*_map_data.js` file + a matching entry in `regions_index.js` flows into `INDEX` (line 272), is picked up by `refreshIndex()` (lines 658–676) which invalidates cached merges and rebuilds the nav, and renders via `navEntries()`'s "Other areas" catch-all (lines 690–692) with **no HTML edit**. `loadRegion` resolves the file by `entry.file` and normalises `boroughs→districts` / injects a view if absent (lines 469–479). So *discovery* is genuinely zero-touch.
- **Caveat:** to place the region *under the correct nation/parent* in the tree (rather than the flat "Other areas" bucket), and to give it a curated centre/zoom/cluster set, you must edit the `NAV` array (lines 285–345) and — for combine regions — the hand-tuned `center`/`zoom`. And whole-nation dedup accuracy depends on the *pipeline's* region-boundary definitions, not the viewer. So the honest claim is: **"drop a data file + register it in the index" makes a region appear and work; correct hierarchical placement and dedup still require pipeline + one NAV edit.**

The single-file 64 KB HTML is otherwise readable and well-sectioned; the main maintainability risk is the absence of any test to catch the class of count bug documented above when regions are added or boundaries redrawn.

---

## 4. Engineering effort embodied (competent senior dev)

Estimates are for a competent senior building this from a clear brief, excluding ongoing per-region sweeps. Ranges reflect the taxonomy/geocoding being the judgement-heavy wildcards.

### Viewer (`regions-map.html`) — ~10 person-days

| Component | Days | Notes |
|---|---:|---|
| deck.gl layer orchestration, GPU attribute transitions, crossfade + ghost layers | 2.0 | The hardest front-end work; requires deck.gl fluency |
| van Wijk–Nuij flight-path implementation | 1.5 | Find paper, implement in mercator world coords, debug the pure-zoom edge case |
| Region tree: lazy hierarchy, combine/merge/dedup, England/UK super-groups, incremental refresh | 2.5 | The count-bug lives here; genuinely intricate |
| Parsing/caching, memoised visibility, chip counts, search | 1.0 | |
| Panel, cards, key-gate, graceful-degrade (authFailure/MutationObserver) | 1.5 | |
| CSS design system, dark basemap style, curated CVD-safe palette | 1.0 | Palette *validator* is separate tooling, not in file |
| Integration, perf tuning (dpr cap, throttle, idle prefetch), cross-browser | 1.0 | |
| **Viewer subtotal** | **~10** | 1,205 JS + 210 CSS/HTML lines |

### Data pipeline (implied — not in the file, required to produce the 67 data files + index) — ~15 person-days

| Component | Days | Notes |
|---|---:|---|
| Companies House bulk register ingest (~5 GB / ~5M companies): parse, filter to creative SIC | 2.5 | |
| SIC 2007 → "Soundings operational taxonomy" mapping (proprietary, multi-label per company) | 4.0 | Taxonomy *design* is the expensive, judgement-heavy part; wide range (3–6d) |
| Postcode geocoding via postcodes.io → registered-office centroids for 745k, caching, invalid-postcode handling | 1.5 | |
| Size-tier classification from accounts filing category (unfiled/dormant/micro/small/large) | 1.0 | |
| Per-region file generation: postcode→region assignment, "Fringe" over-approximation logic, **dedup on company number**, compact tuple encoding, district assignment, index emit (`soundings.reader.region_mapdata`, referenced line 959) | 3.5 | The region-boundary + dedup system the viewer relies on |
| Cluster/borough curation (hand-tuned lat/lng, dozens per region across 60 regions) | 1.5 | Visible in `NW_REGIONS`/`LONDON_BOROUGHS`; manual cartographic work |
| Orchestration, incremental SHA tracking, regen tooling, palette validator | 1.0 | |
| **Pipeline subtotal** | **~15** | |

### Totals

| | Person-days |
|---|---:|
| **Viewer** | **~10** (range 9–11) |
| **Data pipeline** | **~15** (range 13–18) |
| **Total embodied effort** | **~25** (range 22–29) |

~25 person-days ≈ 5 focused senior-engineer weeks, excluding the ongoing "add regions as asked" operational work and the design/UX iteration implied by the validated palette and curated basemap. The taxonomy mapping is the single largest uncertainty; if the "Soundings operational taxonomy" is a substantial curated asset rather than a thin SIC lookup, pipeline effort could run higher.

---

## Summary for value estimate

- **Craft rating: 7/10.** High craft in the rendering/animation/data-format layers (deck.gl transitions, van Wijk flight math, tuple encoding, graceful degradation, honest provenance). Held back by zero test seams, an unhandled-rejection path on failed leaf loads, a11y gaps, and — most importantly — a **knowingly-shipped count inconsistency** the author's own comment admits.
- **Root cause of England discrepancy:** `updateNav()` lines 838–843 sum per-region index counts (mixing whole-region files for slug nations with child-sums for the five `combine` regions) with **no dedup**, reproducing 689,793 exactly; the drill value 686,026 comes from `mergePayloads()` lines 538–554, which **dedups on Companies House number** (`row[6]`, line 550). The 3,767 delta is double-counted overlaps (e.g. `north_devon` ⊂ `devon`, and Humber unitaries across the Yorkshire/East-Midlands boundary).
- **Total embodied effort:** **~25 person-days** — **~10 viewer** + **~15 data pipeline**.
