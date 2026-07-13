# Accessibility Findings — Soundings Regions-Map

**Target:** https://soundings-23l.pages.dev/regions-map
**Standard:** WCAG 2.2 Level AA
**Method:** Static source audit of `data/regions-map.html` (single-file vanilla-JS app, ~1,205 lines inline JS) + Lighthouse `report.json` audits. No browser was driven; findings are anchored to quoted source lines.
**Date:** 2026-07-13

---

## Maturity score: **2 / 10**

The app uses real `<button>` elements in exactly two places (the gate's "Load map" and the fly-to cluster chips) and gets the free wins (`<html lang="en">`, `<title>`, HTML-escaped output), but **every bespoke control — view modes, the entire region tree, all filter chips, "all/none" toggles, search results, and the detail card — is a `<div>`/`<span>` with an `onclick` and nothing else: no `role`, no `tabindex`, no keyboard handler.** There are **zero** ARIA attributes, **zero** `<label>` elements, **zero** live regions, and **no landmarks** in the whole document. Keyboard-only and screen-reader users cannot switch views, drill the tree, filter, or read a company. The Lighthouse score of 94 is an artefact of the auditor seeing these widgets as inert text.

---

## Evidence: the structural gaps (grep over `regions-map.html`)

```
$ grep -Ei 'aria-|role=|tabindex'  regions-map.html   → (none found)
$ grep -Ei '<label'                regions-map.html   → (no <label> elements)
$ grep -Ei '<main|<nav|<header|<footer|<section' ...  → (none)
$ grep -Ei '<button'               regions-map.html   → 1 static (#go); .fly buttons created in JS
$ grep -Ei '<h1|<h2|<h3'           regions-map.html   → h2 (L155, gate) BEFORE h1 (L169, panel), h3 (L1331)
```

**Correction to the brief:** the app has **no aria-live region at all** (the fleet brief's "there is one live region" does not hold — see F10). Nothing the app updates (the "Showing N of M" count, search results, the active region, the detail card) is announced to assistive technology.

---

## Findings by severity

| # | Severity | Finding | WCAG 2.2 |
|---|----------|---------|----------|
| F1 | 🔴 Critical | View-mode switcher is 3 `<div>`s, mouse-only | 2.1.1, 4.1.2 |
| F2 | 🔴 Critical | Region tree (leaf/group/UK/borough rows) mouse-only, no tree/expand semantics | 2.1.1, 4.1.2, 1.3.1 |
| F3 | 🔴 Critical | Filter chips (subsector/size/district) mouse-only toggles, state via opacity only | 2.1.1, 4.1.2, 1.4.1 |
| F4 | 🔴 Critical | Map canvas has no non-visual alternative; per-point data is hover-only | 1.1.1, 2.1.1 |
| F5 | 🟠 Serious | "all / none" toggles are `<span>`s, mouse-only | 2.1.1, 4.1.2 |
| F6 | 🟠 Serious | Search results are `<div>`s, not keyboard-operable, no list semantics | 2.1.1, 4.1.2, 1.3.1 |
| F7 | 🟠 Serious | Detail card: `<span>` close, no focus management, no dialog semantics, no Esc | 2.1.1, 4.1.2, 2.4.3 |
| F8 | 🟠 Serious | Contrast fail — `--muted` count/label text (Lighthouse-confirmed 4.37:1) | 1.4.3 |
| F9 | 🟠 Serious | Search input labelled by placeholder only | 1.3.1, 3.3.2, 4.1.2 |
| F10 | 🟠 Serious | No status messages — count/results/region changes never announced | 4.1.3 |
| F11 | 🟡 Moderate | No visible focus indicator for custom widgets | 2.4.7 |
| F12 | 🟡 Moderate | No `<main>` / no landmark structure (Lighthouse `landmark-one-main`) | 1.3.1 |
| F13 | 🟡 Moderate | Heading order — H2 precedes H1; orphan H3 | 1.3.1 |
| F14 | 🟡 Moderate | API-key input is `type=password`, unlabelled; gate is a non-dialog modal | 1.3.1, 3.3.2, 4.1.2 |
| F15 | 🔵 Minor | New-window link + icon-only glyphs (✕, ▶, ↗) lack text equivalents | 3.2.4, 4.1.2 |

**Counts — Critical: 4 · Serious: 6 · Moderate: 4 · Minor: 1 · Total: 15**

---

## Shared remediation helper (add once, used by F1–F7)

Every custom-control fix below builds on one helper written in the app's own idiom (`document.createElement`, `.onclick`, template literals). Add it near the top of the `<script>`:

```js
/* ===== a11y: turn a div/span into a real, keyboard-operable control ===== */
function actuate(el, handler, opts = {}){
  const { role = "button", checked = null, expanded = null } = opts;
  el.setAttribute("role", role);
  if (!el.hasAttribute("tabindex")) el.tabIndex = 0;
  if (checked  !== null) el.setAttribute("aria-checked",  String(checked));
  if (expanded !== null) el.setAttribute("aria-expanded", String(expanded));
  el.addEventListener("keydown", e => {
    if (e.key === "Enter" || e.key === " " || e.key === "Spacebar"){
      e.preventDefault();
      handler(e);
    }
  });
  el.addEventListener("click", handler);
  return el;
}
```

And a focus indicator (F11) — add to the `<style>` block so every newly-focusable control shows focus:

```css
/* a11y: visible keyboard focus for all custom controls */
.mode:focus-visible, .grp:focus-visible, .leaf:focus-visible,
.chip:focus-visible, .hit:focus-visible, .lnk:focus-visible,
#card .x:focus-visible, .grp .chev:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
  border-radius: 7px;
}
/* respect users who reduce motion — the app animates crossfades/flights */
@media (prefers-reduced-motion: reduce){
  .grp .chev { transition: none; }
}
```

---

## F1 — 🔴 Critical — View-mode switcher is mouse-only

**WCAG:** 2.1.1 Keyboard (A), 4.1.2 Name, Role, Value (A)

**Source (markup, L176–180):**
```html
<div class="modes" id="modes">
  <div class="mode" data-m="overview">Overview</div>
  <div class="mode on" data-m="sectors">Subsectors</div>
  <div class="mode" data-m="size">Size &amp; scale</div>
</div>
```
**Source (handler, L1226–1235):**
```js
document.querySelectorAll(".mode").forEach(el=>{
  el.onclick = ()=>{
    document.querySelectorAll(".mode").forEach(x=>x.classList.remove("on"));
    el.classList.add("on");
    S.mode = el.dataset.m; ...
  };
});
```

**Impact:** These are three mutually-exclusive views (a radio group). They are unreachable by Tab, cannot be triggered by Enter/Space, and a screen reader announces three unlabelled text strings with no role and no indication which is selected. Keyboard and SR users are locked into the default `sectors` mode.

**Fix — semantic radiogroup with arrow-key roving (drop-in replacement for L1226–1235):**
```js
const modeBox = document.getElementById("modes");
modeBox.setAttribute("role", "radiogroup");
modeBox.setAttribute("aria-label", "Map view mode");
const modeEls = [...document.querySelectorAll(".mode")];
const selectMode = el => {
  modeEls.forEach(x=>{
    const on = x === el;
    x.classList.toggle("on", on);
    x.setAttribute("aria-checked", String(on));
    x.tabIndex = on ? 0 : -1;          // roving tabindex: one stop for the group
  });
  S.mode = el.dataset.m;
  document.getElementById("modenote").textContent = MODE_NOTES[S.mode];
  closeCard();
  render();
};
modeEls.forEach((el, i)=>{
  el.setAttribute("role", "radio");
  el.setAttribute("aria-checked", String(el.classList.contains("on")));
  el.tabIndex = el.classList.contains("on") ? 0 : -1;
  el.addEventListener("click", ()=>selectMode(el));
  el.addEventListener("keydown", e=>{
    if (e.key === "Enter" || e.key === " "){ e.preventDefault(); selectMode(el); return; }
    if (e.key === "ArrowRight" || e.key === "ArrowDown"){
      e.preventDefault(); const n = modeEls[(i+1)%modeEls.length]; n.focus(); selectMode(n);
    }
    if (e.key === "ArrowLeft" || e.key === "ArrowUp"){
      e.preventDefault(); const n = modeEls[(i-1+modeEls.length)%modeEls.length]; n.focus(); selectMode(n);
    }
  });
});
```
The `modenote` text (`MODE_NOTES[...]`) is a good accessible description — reference it with `aria-describedby="modenote"` on the radiogroup for even richer output (add `id="modenote"` already exists on L181).

---

## F2 — 🔴 Critical — Region tree is mouse-only, no expand/select semantics

**WCAG:** 2.1.1 Keyboard (A), 4.1.2 Name, Role, Value (A), 1.3.1 Info & Relationships (A)

**Source — leaf rows (L702–716):**
```js
function leafRow(label, slug){
  const div = document.createElement("div");
  div.dataset.r = slug; ...
  div.className = "leaf";
  div.innerHTML = `<span>${esc(label)}</span>` + (n ? `<span class="n">${n.toLocaleString()}</span>` : "");
  div.onclick = ()=>{ if (slug !== S.region) switchRegion(slug); };
  return div;
}
```
**Source — group rows (L718–738):**
```js
grp.className = "grp";
grp.innerHTML = `<span class="chev">▶</span><span>${esc(name)}</span><span class="n"></span>`;
...
grp.querySelector(".chev").onclick = e=>{ e.stopPropagation(); toggle(); };
grp.onclick = ()=>{ grp.classList.add("open"); kids.classList.add("open"); if (onSelect) onSelect(); else toggle(); };
```
Also affected: the UK row (L804–809, `uk.onclick = switchUK`) and London borough rows (`boroughRow`, L771–778).

**Impact:** The primary navigation of the entire app — UK → nations → English regions → 60+ sub-regions — cannot be operated without a mouse. Groups expand/collapse with no `aria-expanded`, so a screen reader gives no hint that "North West" is a container or whether it is open. Selected region (`.on`) is signalled by background colour only (no `aria-current`). The tree has no `role="tree"`, so it reads as a flat pile of text.

**Fix — `leafRow` (replace L702–716):**
```js
function leafRow(label, slug){
  const div = document.createElement("div");
  div.dataset.r = slug;
  if (!available.has(slug)){
    div.className = "leaf soon";
    div.setAttribute("role", "treeitem");
    div.setAttribute("aria-disabled", "true");
    div.innerHTML = `<span>${esc(label)}</span><span class="n">soon</span>`;
    return div;                                   // not focusable: not yet loadable
  }
  div.className = "leaf";
  const n = countOf(slug);
  div.innerHTML = `<span>${esc(label)}</span>` +
    (n ? `<span class="n">${n.toLocaleString()}</span>` : "");
  actuate(div, ()=>{ if (slug !== S.region) switchRegion(slug); }, { role: "treeitem" });
  div.setAttribute("aria-current", slug === S.region ? "true" : "false");
  return div;
}
```
**Fix — `groupRow` (replace L718–738):**
```js
function groupRow(name, container, onSelect){
  const grp = document.createElement("div");
  grp.className = "grp";
  grp.dataset.g = name;
  grp.innerHTML = `<span class="chev" aria-hidden="true">▶</span>` +
                  `<span>${esc(name)}</span><span class="n"></span>`;
  const kids = document.createElement("div");
  kids.className = "kids";
  kids.setAttribute("role", "group");
  const kidsId = "kids-" + name.replace(/\W+/g, "-").toLowerCase();
  kids.id = kidsId;

  const setOpen = open => {
    grp.classList.toggle("open", open);
    kids.classList.toggle("open", open);
    grp.setAttribute("aria-expanded", String(open));
  };
  actuate(grp, ()=>{
    setOpen(true);
    if (onSelect) onSelect(); else setOpen(!grp.classList.contains("open"));
  }, { role: "treeitem", expanded: false });
  grp.setAttribute("aria-controls", kidsId);

  container.appendChild(grp);
  container.appendChild(kids);
  return kids;
}
```
Apply the same `actuate(div, handler, { role: "treeitem" })` treatment to `boroughRow` (L771–778) and the UK row (L804–809), and set `role="tree"` + `aria-label="Regions and areas"` on `#regionnav` once in `buildNav()` (L801–816):
```js
const nav = document.getElementById("regionnav");
nav.setAttribute("role", "tree");
nav.setAttribute("aria-label", "Regions and areas");
```
Keep `updateNav()` (L818–866) in sync by mirroring the `.on` class to `aria-current`:
```js
document.querySelectorAll("#regionnav .leaf").forEach(el=>{
  const on = el.dataset.r === S.region;
  el.classList.toggle("on", on);
  el.setAttribute("aria-current", on ? "true" : "false");
  ...
});
```
*(Full APG arrow-key tree navigation is the gold standard; the above satisfies 2.1.1/4.1.2/1.3.1 at AA. Roving arrow keys are a worthwhile enhancement given the depth.)*

---

## F3 — 🔴 Critical — Filter chips are mouse-only; state conveyed by opacity only

**WCAG:** 2.1.1 Keyboard (A), 4.1.2 Name, Role, Value (A), 1.4.1 Use of Color (A)

**Source (`chipRow`, L1209–1220):**
```js
function chipRow(container, key, label, color, set, value){
  const div = document.createElement("div");
  div.className = "chip";
  div.innerHTML = (color ? `<span class="dot" style="background:${color}"></span>` : "") +
    `<span>${esc(label)}</span><span class="n" data-count="${key}:${esc(value)}"></span>`;
  div.onclick = ()=>{
    if (set.has(value)) set.delete(value); else set.add(value);
    div.classList.toggle("off", !set.has(value));
    render();
  };
  container.appendChild(div);
}
```
Off-state style (L89): `.chip.off { opacity:.38; }`.

**Impact:** Every subsector, size-tier and district filter is an unreachable toggle. On/off state is expressed purely by `opacity:.38` — no text, no `aria-checked` — so a screen reader cannot tell an active filter from an inactive one, and the low-opacity "off" text also drops below the contrast floor. This is core functionality (filtering 745k records) that keyboard and SR users cannot use.

**Fix (replace `chipRow`):**
```js
function chipRow(container, key, label, color, set, value){
  const div = document.createElement("div");
  div.className = "chip";
  const isOn = set.has(value);
  div.classList.toggle("off", !isOn);
  div.innerHTML = (color ? `<span class="dot" style="background:${color}" aria-hidden="true"></span>` : "") +
    `<span>${esc(label)}</span><span class="n" data-count="${key}:${esc(value)}"></span>`;
  actuate(div, ()=>{
    const nowOn = !set.has(value);
    if (nowOn) set.add(value); else set.delete(value);
    div.classList.toggle("off", !nowOn);
    div.setAttribute("aria-checked", String(nowOn));
    render();
  }, { role: "checkbox", checked: isOn });
  container.appendChild(div);
}
```
`role="checkbox"` + `aria-checked` makes the state programmatic — this fixes 1.4.1 as well, because state no longer depends on the opacity cue. As a belt-and-braces visual improvement, keep the "off" chip legible by not relying solely on transparency:
```css
.chip.off { opacity:1; color:var(--muted); }
.chip.off .dot { opacity:.35; box-shadow:0 0 0 1px var(--hairline) inset; }
.chip[aria-checked="true"] .dot::after { /* optional tick affordance */ }
```

---

## F4 — 🔴 Critical — Map canvas has no non-visual alternative; point data is hover-only

**WCAG:** 1.1.1 Non-text Content (A), 2.1.1 Keyboard (A)

**Source:** `#map` is an empty `<div>` (L220) handed to Google Maps (L979) with a deck.gl WebGL overlay (L992). All 745,485 plotted points, the colour→subsector encoding, the size→scale encoding and the cluster hotspots exist only as rendered pixels. The per-company tooltip is mouse-hover only:
```js
overlay = new deck.GoogleMapsOverlay({ getTooltip: tooltip, ... });   // L992
function tooltip({ object }){ ... }                                    // L1144
```
and the detail card only opens from a canvas `onClick` (L1067) or a search hit (L1265).

**Impact:** A screen-reader user reaches an unlabelled empty region where the app's core content lives. There is no text summary of what the map shows, no keyboard path to any data point (deck.gl's canvas is not focusable), and the hover tooltip is unreachable without a pointer. The colour-coded subsector meaning has no non-visual equivalent on the map itself.

**Fix — three parts:**

1. Label the map region and give it a live text summary (add to `#map`, and update in `render()`):
```html
<main id="map" role="application" aria-label="Interactive map of registered creative businesses" aria-describedby="map-summary"></main>
<p id="map-summary" class="visually-hidden"></p>
```
```js
// in render(), after computing n:
document.getElementById("map-summary").textContent =
  `${D.region}: showing ${n.toLocaleString()} of ${companies.length.toLocaleString()} ` +
  `registered creative businesses, coloured by ${S.mode === "sectors" ? "subsector" :
   S.mode === "size" ? "size tier" : "density"}. Use the region tree and filters to explore; ` +
  `search by company name for a specific record.`;
```

2. Provide a keyboard-reachable data equivalent. The region tree already exposes regional counts as text (good) — extend that principle to the plotted layer with a "list view" of the current filtered set (top-N by size tier), rendered as a real `<ul>`/`<table>` that mirrors what the dots encode. Minimum viable: the `LARGE` (sector-prime) list is already computed (`e.large`, L436) — surface it as an accessible list under a "Notable firms in view" disclosure.

3. Add the visually-hidden utility used above:
```css
.visually-hidden { position:absolute!important; width:1px; height:1px; padding:0; margin:-1px;
  overflow:hidden; clip:rect(0 0 0 0); white-space:nowrap; border:0; }
```

---

## F5 — 🟠 Serious — "all / none" toggles are unreachable `<span>`s

**WCAG:** 2.1.1 Keyboard (A), 4.1.2 Name, Role, Value (A)

**Source (markup L190 & L198):**
```html
<div class="sect" id="sectorhead">Subsectors <span class="lnk" id="secall">all / none</span></div>
...
<div class="sect">District <span class="lnk" id="dall">all / none</span></div>
```
**Source (handler L1238–1248):** `document.getElementById(id).onclick = ...`

**Impact:** Bulk select/deselect of every subsector and every district is mouse-only, and reads as ambiguous text "all / none" with no role. The `.lnk` is styled as a link (accent colour) but is neither a link nor a button.

**Fix (in `allify`, wrap the existing handler):**
```js
const allify = (id, setOf, allOf, containerId, label)=>{
  const el = document.getElementById(id);
  el.setAttribute("aria-label", label);
  actuate(el, ()=>{
    const set = setOf(), all = allOf();
    const on = set.size < all.length;
    set.clear(); if (on) all.forEach(v=>set.add(v));
    document.querySelectorAll(`#${containerId} .chip`).forEach(c=>{
      c.classList.toggle("off", !on);
      c.setAttribute("aria-checked", String(on));
    });
    render();
  }, { role: "button" });
};
allify("secall", ()=>S.groups, ()=>GROUPS, "sectorchips", "Toggle all subsectors on or off");
allify("dall",  ()=>S.districts, ()=>D.districts, "districtchips", "Toggle all districts on or off");
```
(Note the added `aria-checked` sync so F3's chip states stay correct after a bulk toggle.)

---

## F6 — 🟠 Serious — Search results not keyboard-operable, no list semantics

**WCAG:** 2.1.1 Keyboard (A), 4.1.2 Name, Role, Value (A), 1.3.1 Info & Relationships (A)

**Source (L1250–1271):**
```js
input.addEventListener("input", ()=>{
  const q = input.value.trim().toLowerCase();
  results.innerHTML = "";
  if (q.length < 2) return;
  const hits = []; ...
  hits.forEach(c=>{
    const div = document.createElement("div");
    div.className = "hit";
    div.innerHTML = `<span>${esc(c.name)}</span><span class="kind">${esc(c.primary)}</span>`;
    div.onclick = ()=>{ results.innerHTML = ""; input.value = c.name; flyTo(...); showCompanyCard(c); };
    results.appendChild(div);
  });
});
```

**Impact:** Search is the one keyboard path to per-company data (which matters given F4), but the results themselves are mouse-only `<div>`s with no list/option roles, and their appearance is silent (see F10). A keyboard user can type a query but cannot select any result.

**Fix — give the results listbox semantics and make each hit operable:**
```js
results.setAttribute("role", "listbox");
results.setAttribute("aria-label", "Company search results");
// input wiring for combobox (see F9 for the label):
input.setAttribute("role", "combobox");
input.setAttribute("aria-expanded", "false");
input.setAttribute("aria-controls", "results");
input.setAttribute("aria-autocomplete", "list");

input.addEventListener("input", ()=>{
  const q = input.value.trim().toLowerCase();
  results.innerHTML = "";
  if (q.length < 2){ input.setAttribute("aria-expanded", "false"); announce(""); return; }
  const hits = [];
  for (const c of companies){ if (hits.length >= 9) break; if (c.name.toLowerCase().includes(q)) hits.push(c); }
  hits.forEach(c=>{
    const div = document.createElement("div");
    div.className = "hit";
    div.innerHTML = `<span>${esc(c.name)}</span><span class="kind">${esc(c.primary)}</span>`;
    actuate(div, ()=>{
      results.innerHTML = ""; input.value = c.name;
      input.setAttribute("aria-expanded", "false");
      flyTo(c.la, c.lo, 16.5); showCompanyCard(c);
    }, { role: "option" });
    results.appendChild(div);
  });
  input.setAttribute("aria-expanded", hits.length ? "true" : "false");
  announce(hits.length
    ? `${hits.length} result${hits.length === 1 ? "" : "s"} for “${input.value.trim()}”`
    : `No matches for “${input.value.trim()}”`);   // announce() from F10
});
```
(Down-arrow roving between options + `aria-activedescendant` is the full combobox pattern; the above already restores keyboard operability and status.)

---

## F7 — 🟠 Serious — Detail card: `<span>` close, no focus management, no dialog semantics

**WCAG:** 2.1.1 Keyboard (A), 4.1.2 Name, Role, Value (A), 2.4.3 Focus Order (A)

**Source (L1324–1341):**
```js
const card = document.getElementById("card");
function closeCard(){ card.style.display = "none"; }
function showCompanyCard(c){
  card.innerHTML = `<span class="x">✕</span> <h3>${esc(c.name)}</h3> ...`;
  card.querySelector(".x").onclick = closeCard;
  card.style.display = "block";
}
```

**Impact:** The company card is shown/hidden via `display` with no accessibility contract: the close control is a `<span>✕</span>` (not focusable, not labelled — a screen reader hears only "✕"), focus is never moved into the card when it opens, focus is never restored when it closes, there is no `role="dialog"`/`aria-labelledby`, and Esc does not dismiss it. A keyboard user who reaches the card via a fixed search flow is stranded, and an SR user is not told the card appeared.

**Fix (replace L1324–1341):**
```js
const card = document.getElementById("card");
card.setAttribute("role", "dialog");
card.setAttribute("aria-modal", "false");     // non-blocking overlay; panel stays usable
card.setAttribute("aria-labelledby", "card-title");
let _cardReturnFocus = null;

function closeCard(){
  card.style.display = "none";
  card.removeAttribute("aria-hidden");
  if (_cardReturnFocus && document.contains(_cardReturnFocus)) _cardReturnFocus.focus();
  _cardReturnFocus = null;
}

function showCompanyCard(c){
  _cardReturnFocus = document.activeElement;
  card.innerHTML =
    `<button class="x" aria-label="Close company details" type="button">✕</button>
     <h3 id="card-title">${esc(c.name)}</h3>
     <div>${c.groups.map(g=>`<span class="badge"><span class="bd" style="background:${GROUP_COLOR[g]}" aria-hidden="true"></span>${esc(g)}</span>`).join("")}</div>
     <div class="row"><span class="k">Size:</span> ${TIER_LABEL[c.tier]}</div>
     <div class="row"><span class="k">District:</span> ${esc(c.d)}${c.yr ? ` · <span class="k">Incorporated:</span> ${c.yr}` : ""}</div>
     <div class="row"><a href="https://find-and-update.company-information.service.gov.uk/company/${esc(c.num)}" target="_blank" rel="noopener">Companies House record (opens in a new tab) ↗</a></div>
     <div class="prov">Source: Companies House bulk register (July 2026). Position is the registered-office postcode centroid — claimed, not verified. Size tier from the latest filed accounts category.</div>`;
  card.querySelector(".x").addEventListener("click", closeCard);
  card.style.display = "block";
  announce(`Company details: ${c.name}`);       // F10
  card.querySelector(".x").focus();              // move focus into the dialog
}

// Esc closes the card (add once, near the other keydown listeners ~L1410)
document.addEventListener("keydown", e=>{
  if (e.key === "Escape" && card.style.display === "block") closeCard();
});
```
The `.x` selector CSS at L114–116 already styles `#card .x`; a `<button>` inherits it if you add `background:none;border:0;font:inherit;` — include:
```css
#card .x { background:none; border:0; font-family:inherit; padding:0; }
```

---

## F8 — 🟠 Serious — Contrast failure on muted count/label text

**WCAG:** 1.4.3 Contrast (Minimum) (AA) — **Lighthouse-confirmed** (`color-contrast`, score 0)

**Source:** `--muted: #898781;` (L16) rendered on `--surface-2: #232322;` — Lighthouse measured `#regionnav .leaf span.n` at **4.37:1** (needs 4.5:1) at 10.5px/7.9pt. The same `--muted` on `--surface: #1a1a19` computes ~4.86:1 (marginal pass) and on the selected-row blue `#0f2748` ~4.17:1 (**fail**). `--muted` is used far beyond the one node Lighthouse flagged: `.sect` headers, `.grp .n` / `.leaf .n` / `.chip .n` counts (L67/72/92), `#crumb` (L45), `.note` (L101), `.footer` (L104), `#card .prov` (L124), `#results .hit .kind` (L83).

**Impact:** Low-vision users cannot reliably read region counts, breadcrumb, filter counts, provenance and footnotes — much of the app's quantitative substance.

**Fix (one line, L16):**
```css
--muted: #9a988f;   /* was #898781 — now ≈5.4:1 on surface-2, ≈5.2:1 on #0f2748, ≈6:1 on surface */
```
This clears 4.5:1 against all three background surfaces the token is drawn on. (Verified by relative-luminance computation for each pairing.) No layout change; purely the CSS variable.

---

## F9 — 🟠 Serious — Search input labelled by placeholder only

**WCAG:** 1.3.1 (A), 3.3.2 Labels or Instructions (A), 4.1.2 (A)

**Source (L184):**
```html
<input id="search" type="text" placeholder="Company name…" autocomplete="off">
```

**Impact:** Placeholder text is not a label: it disappears on input and is inconsistently exposed by assistive tech. A screen-reader user tabbing here hears "edit text" with no purpose. (Same defect applies to `#gkey`, see F14.)

**Fix — visible or visually-hidden label + accessible name:**
```html
<label for="search" class="sect">Search companies</label>
<input id="search" type="text" placeholder="Company name…" autocomplete="off"
       aria-label="Search companies by name">
```
The existing `<div class="sect">Search</div>` at L183 can simply become `<label class="sect" for="search">Search</label>` — no visual change.

---

## F10 — 🟠 Serious — No status messages; nothing the app updates is announced

**WCAG:** 4.1.3 Status Messages (AA)

**Source:** The "Showing N of M" text is written via `innerHTML` with no live region (L1173–1174); search results appear silently (L1254); region/view changes update the DOM with no announcement. Grep confirms **no `aria-live`, `role="status"`, or `role="alert"` anywhere** — this corrects the brief's "there is one live region."

**Impact:** A screen-reader user who toggles a filter, runs a search, or switches region gets no feedback that anything changed — the result count, the new company total, and the number of matches are all invisible to them.

**Fix — add one polite live region and a helper, then call it from `render()` and search:**
```html
<!-- put just inside #panel, e.g. after the crumb (L168) -->
<div id="sr-status" role="status" aria-live="polite" class="visually-hidden"></div>
```
```js
let _lastAnnounce = "";
function announce(msg){
  if (msg === _lastAnnounce) return;             // avoid duplicate SR chatter
  _lastAnnounce = msg;
  document.getElementById("sr-status").textContent = msg;
}
// in render(), after updating #counts:
announce(`Showing ${n.toLocaleString()} of ${companies.length.toLocaleString()} businesses in ${D.region}.`);
```
`announce()` is reused by F6 (search matches) and F7 (card opened). Use `class="visually-hidden"` from F4 so the region is exposed to AT but not visually rendered.

---

## F11 — 🟡 Moderate — No visible focus indicator for custom controls

**WCAG:** 2.4.7 Focus Visible (AA)

**Source:** Only `#search:focus` has a focus style (L78, border colour change). The `.mode`, `.grp`, `.leaf`, `.chip`, `.hit`, `.lnk` and card-close elements have no `:focus`/`:focus-visible` rule — and until F1–F7 they cannot receive focus at all. Once those fixes add `tabindex`, a visible indicator becomes mandatory.

**Fix:** the `:focus-visible` CSS block in the **Shared remediation helper** section above covers every newly-focusable control. Ship it together with F1–F7.

---

## F12 — 🟡 Moderate — No `<main>` / no landmark structure

**WCAG:** 1.3.1 Info & Relationships (A) — **Lighthouse-confirmed** (`landmark-one-main`, score 0)

**Source:** The document is `<div id="gate">`, `<div id="panel">`, `<div id="card">`, `<div id="map">` — no `<main>`, `<nav>`, `<header>` or landmark roles (grep confirms none).

**Impact:** Screen-reader users cannot jump to the main content or the navigation with landmark shortcuts; there is no "skip to content" affordance, so they must traverse the entire control panel linearly before reaching anything.

**Fix:**
```html
<div id="panel" role="region" aria-label="Regions, views and filters"> ... </div>
<main id="map" aria-label="Interactive map of registered creative businesses"></main>
```
Google Maps accepts any element passed to `new Map(document.getElementById("map"), …)` (L979), so promoting `#map` from `<div>` to `<main>` is safe. This satisfies `landmark-one-main` and pairs with the `#map` labelling in F4.

---

## F13 — 🟡 Moderate — Heading order: H2 precedes H1; orphan H3

**WCAG:** 1.3.1 Info & Relationships (A)

**Source (DOM order):** `<h2>` gate title (L155) comes **before** `<h1 id="title">` (L169); the card `<h3>` (L1331) has no H2 above it in its own context. Lighthouse did not flag this only because the gate is `display:none` by default — but the gate is shown whenever the map is unavailable (`showUnavailable`, L892) or the user reveals the own-key path, at which point an H2 renders above a still-present H1.

**Impact:** Heading-navigation users get a broken outline (a level-2 before the level-1) whenever the gate is visible.

**Fix (minimal, no visual change):** the gate is `position:absolute; inset:0` (L128) so its DOM position does not affect layout — move the `<div id="gate">…</div>` block to sit **after** `<div id="panel">…</div>`. The document H1 (`#title`) then precedes the gate's H2. Alternatively, since the gate is a modal, give it dialog semantics (F14) and set the panel/map `aria-hidden="true"` while it is open, which resets the heading context.

---

## F14 — 🟡 Moderate — API-key input is `type=password`, unlabelled; gate is a non-dialog modal

**WCAG:** 1.3.1 (A), 3.3.2 Labels or Instructions (A), 4.1.2 (A)

**Source (L153–165):**
```html
<div id="gate">
  <div class="box">
    <h2>Soundings — Regional Maps</h2>
    <p id="gatemsg">…needs a Google Maps API key…</p>
    <input id="gkey" type="password" placeholder="Google Maps API key">
    <button id="go">Load map</button>
    ...
```

**Impact:** (a) `type="password"` for a non-secret API key triggers password-manager interference and the browser's "password field not in a form" notice, and masks input the user may want to verify; (b) the input has no `<label>` (placeholder only) — same defect class as F9; (c) `#gate` is a full-screen modal with no `role="dialog"`, no `aria-modal`, no focus trap and no accessible name, so when it appears (e.g. quota exhaustion) focus is not moved into it and background content is not hidden.

**Fix:**
```html
<div id="gate" role="dialog" aria-modal="true" aria-labelledby="gate-h" aria-describedby="gatemsg">
  <div class="box">
    <h2 id="gate-h">Soundings — Regional Maps</h2>
    <p id="gatemsg">…</p>
    <label for="gkey" class="visually-hidden">Google Maps API key</label>
    <input id="gkey" type="text" inputmode="text" autocomplete="off" spellcheck="false"
           placeholder="Google Maps API key" aria-describedby="gatemsg">
    <button id="go" type="button">Load map</button>
```
When showing the gate (`showUnavailable`, L892, and the boot path), move focus to `#gkey` (or `#go`), and set `document.getElementById("panel").setAttribute("aria-hidden","true")` while it is open (clear on hide). `type="text"` also resolves the Chrome "password field not in a form" console notice noted in the brief.

---

## F15 — 🔵 Minor — New-window link and icon-only glyphs lack text equivalents

**WCAG:** 3.2.4 Consistent Identification / 4.1.2 (A) — best-practice hardening

**Source:** the Companies House link opens a new tab (`target="_blank"`, L1335) with only a "↗" glyph as the cue; the card close is "✕", the tree chevron is "▶" (L722), the size/subsector state is a coloured `.dot` with no text.

**Impact:** Users relying on non-visual output miss the "opens in new tab" behaviour and the meaning of decorative glyphs.

**Fix:** already folded into F2 (`aria-hidden="true"` on the chevron), F3/F7 (`aria-hidden` on `.dot`/`.bd` swatches — the chip/badge text already carries the meaning), and F7 (link text "opens in a new tab" + `aria-label` on the close button). No further work once F2/F3/F7 ship.

---

## Suggested remediation order (ROI)

1. **Add the `actuate()` helper + focus CSS + `.visually-hidden`** (one-time, ~20 lines) — unblocks F1, F2, F3, F5, F6, F7, F11 at once.
2. **F8 contrast** (one-line token change) and **F12 `<main>`** (two attributes) — clear the two Lighthouse-confirmed failures immediately.
3. **F1/F3/F5** (modes, chips, all/none) — restore filtering for keyboard/SR.
4. **F2** (region tree) — restore navigation.
5. **F6/F9/F10** (search + label + live region) — restore the keyboard path to company data.
6. **F7/F14** (dialogs + focus management), **F13** (DOM reorder), **F15** (glyph cleanup).

Total surface: one shared helper plus in-place edits to ~10 existing functions. No framework, no build step — every fix is a drop-in in the site's existing vanilla-JS idiom.
