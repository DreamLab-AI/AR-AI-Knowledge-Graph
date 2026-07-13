# Soundings Regions-Map — QE Evidence Brief
Collected 2026-07-13 via GPU browser sidecar (Chrome, chrome-devtools-mcp). Target: https://soundings-23l.pages.dev/regions-map

## What the product is
"Soundings — Regional Creative Economy Maps": an interactive UK-wide map of **745,485 registered creative businesses** from the Companies House bulk register (July 2026), geocoded to registered-office postcode centroids (postcodes.io, OGL licence), classified by a proprietary "Soundings operational taxonomy" over UK SIC 2007. Deck.gl 9 scatterplot over Google Maps vector basemap. Hierarchical region tree (UK → nations → English regions → 60+ sub-regions e.g. Liverpool City Region 8,966, Greater Manchester 28,105). Three view modes (Overview / Subsectors / Size & scale), full-dataset company search, per-company detail card with Companies House deep link, subsector/size/district filters, fly-to-cluster shortcuts. Footer states this is the multi-region sibling of a "North West flagship map" that carries a curated venues/festivals/institutions layer. Data files carry generated dates 2026-07-09/10; header says evidence graph of 2026-07-06.

## Architecture
- Single-file vanilla-JS app: 64,332-byte HTML with ~1,205 lines of inline JS (saved at data/regions-map.html). No framework, no build system detected.
- deck.gl@9 loaded from unpkg.com CDN (resolved 9.3.6). Google Maps JS API with embedded key AIzaSyCYBppM6M1sUQa3MMjzGQHkAIijXLOPEmI (referrer restrictions unknown). Hidden fallback input#gkey (type=password) lets a user supply their own Maps key ("Load map" button).
- Data: 67 static JS files `data/*_map_data.js`, one per region, each assigning a window global. Compact company tuples: [name, lat, lng, [sectorIdx...], sizeTier, boroughIdx, companiesHouseNo, yearIncorporated]. Region index at data/regions_index.js (window.SOUNDINGS_REGIONS, 60 regions, per-region generated dates + company counts).
- Hosting: Cloudflare Pages (server: cloudflare, h3, CF-NEL). Root `/` is a 174-byte meta-refresh stub to regions-map.html. ALL other paths (including /robots.txt, /sitemap.xml) return the same 174-byte stub with HTTP 200.

## Measured performance (desktop, GPU container, cold-ish cache)
- Total transfer on load: **21.7 MB** (21.4 MB = the 67 data files). Largest: greater_london_map_data.js **6.8 MB** compressed (4.2 s), yorkshire_humber 854 KB, east_midlands_region 797 KB, scotland 735 KB.
- DOMContentLoaded 1.12 s; **load event 19.0 s**. All 67 regions load eagerly on first paint (code comment claims non-NW regions are "loaded on demand" — behaviour contradicts).
- Caching: `cache-control: public, max-age=0, must-revalidate` + ETag on multi-MB data files → revalidation every visit, no immutable caching, no fingerprinted filenames.
- Console: zero JS errors. Chrome WebGL perf warning "GPU stall due to mapping device local buffer" (repeated) during deck.gl rendering of ~745k points.

## Lighthouse (desktop, navigation)
Accessibility **94**, Best Practices **100**, SEO **83**, Agentic-browsing **64**. Full report: data/lighthouse/report.{json,html}.
Failed audits: color-contrast (region-count numerals `#regionnav .leaf span.n`, #898781 on #232322 = 4.37:1 at 7.9pt, needs 4.5); landmark-one-main (no <main>); meta-description missing; robots.txt invalid (5 errors — serves HTML stub); llms.txt absent/invalid.

## Functional validation results (all PASS unless noted)
- Region tree drill-down UK→England→North West: PASS (H1, counts, breadcrumb update; map reframes).
- View mode switch Overview/Subsectors/Size & scale: PASS (legend + description swap). Default mode on load is Subsectors.
- Fly-to-cluster (London): PASS (context-sensitive: cluster list changes per region level).
- Search "aardman" across 745k records: PASS, instant; 9 Aardman group entities returned; clicking result flies to Bristol and opens detail card (chips: Film & TV, Games, Software & Digital, Content & Publishing; Size "Medium/large — sector primes"; District South West; Incorporated 1986; Companies House deep link).
- Subsector filter toggle: PASS mechanically (Music off → SHOWING 670,960 of 745,485; restore exact). NOTE: delta 74,525 ≠ sidebar Music count 102,319 because companies are multi-sector; potential user confusion.
- Fullscreen, zoom controls, keyboard map shortcuts: present (Google Maps built-ins).

## Defects / findings inventory (for fleet triage)
1. DATA CONSISTENCY: UK-level tree shows England 689,793; England drill-down header shows 686,026 (nations then sum exactly to 745,485; UK-level tree does not). Yorkshire tree 33,772 vs DISTRICT panel 33,653; East Midlands 31,514 vs 30,186; South East 105,529 vs 105,283. Footnote about "Fringe" postcode over-approximation may explain sub-region gaps but not the England mismatch.
2. PERF: 21.7 MB eager load / 19 s load event; no on-demand loading despite index design supporting it; no long-term caching; London file 6.8 MB.
3. A11y: sidebar controls (view modes, subsector/size/district filter rows, tree rows, "all / none") are DIVs/SPANs with no role, no tabindex — mouse-only; screen-reader/keyboard users cannot operate filters or tree. Lighthouse 94 understates this because widgets are static text to the auditor. Plus 1 contrast failure; no <main> landmark; H2 precedes H1.
4. MOBILE: fixed 280px sidebar never collapses; at 500px window the map canvas is 220px wide (real 390px phone → ~110px). A media query exists but does not restructure layout.
5. SEO/agentic: no robots.txt/sitemap.xml (stub HTML with 200 for all unknown paths), no meta description, no llms.txt, no Open Graph/social cards. Site invisible-ish to crawlers beyond the one page.
6. SECURITY posture: no CSP, no HSTS, no X-Frame-Options; ACAO `*`; x-content-type-options nosniff present; referrer-policy strict-origin-when-cross-origin present. deck.gl from unpkg (mutable `@9` tag → supply-chain drift risk; resolved 9.3.6 via 302). Google Maps API key in source (normal pattern but restriction status unverifiable). Hidden password-type input for user-supplied key triggers Chrome "password field not in a form" notice — should be type=text with autocomplete=off; no secrets at risk.
7. RESILIENCE: single 174-byte stub for every 404 (no real 404 page); no error UI observed for failed data loads (untested).

## Evidence files
- screenshots/01..09 (desktop states, drilldowns, search, detail card, mobile squeeze)
- data/regions-map.html (full app source), data/regions_index.js, data/lighthouse/report.{json,html}
