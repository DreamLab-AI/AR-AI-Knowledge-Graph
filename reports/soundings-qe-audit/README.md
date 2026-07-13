# Soundings Regions-Map — QE Fleet Audit

Full mapping, validation and QE audit of **https://soundings-23l.pages.dev/regions-map**, run 2026-07-13 via the GPU browser sidecar (chrome-devtools-mcp) plus a five-agent QE fleet.

## Deliverable
- **`report/Soundings-QE-Audit-Report.pdf`** — 20-page A4 report (cover, exec summary, site map, performance, functional validation, consolidated findings register, accessibility, security, code quality, value estimate, screenshot evidence, method). Source: `report/report.html`, press-JPEGs in `report/img/`.

## What was found (headline)
| Dimension | Verdict |
|---|---|
| Functionality | Strong — all views/search/drill-down/detail work, 0 JS errors |
| Engineering craft | 7/10 — deck.gl transitions, van Wijk flight maths, honest provenance; no test seams |
| Data correctness | 1 defect — England 689,793 (UK level) vs 686,026 (drilled); un-deduped sum in `updateNav()` |
| Security | MEDIUM — **confirmed billable Maps-key exposure** (denial-of-wallet), reproduced by probe; XSS correctly defended, no PII |
| Accessibility | 2/10 — every custom control mouse-only; keyboard/SR users locked out |
| Performance | 21.7 MB / 19 s first load; ~2 s achievable (>10×) |
| **Value estimate** | **£45k–£180k**, central **£85k–£110k** as a transferable asset |

## Directory
```
report/
  Soundings-QE-Audit-Report.pdf   final report
  report.html                     report source
  img/                            press-optimised screenshots
screenshots/                      9 full-res PNG captures (desktop states, drilldowns, search, detail card, mobile)
data/
  evidence-brief.md               collector's structured evidence (fleet input)
  regions-map.html                the app as served (64 KB, ~1,205 lines inline JS)
  regions_index.js                region registry (60 regions)
  lighthouse/report.{json,html}   Lighthouse audit
  fleet/
    a11y-findings.md              15 WCAG 2.2 findings + drop-in fixes
    security-findings.md          key-restriction probe, CSP, SRI, headers
    performance-findings.md       19 s root cause + optimisation plan
    code-review-findings.md       craft review + count-discrepancy root cause + effort estimate
    value-estimate.md             three-lens valuation with cited comparables
```

## Site facts
- Interactive UK map of **745,485** Companies House-registered creative businesses (July 2026 register), proprietary "Soundings taxonomy" over UK SIC 2007, postcode-centroid geocoding.
- Single-file vanilla-JS app + deck.gl@9 (unpkg) over Google Maps; 67 static per-region data files (21.4 MB). Cloudflare Pages. One real route (`/regions-map`); all other paths return a 174-byte stub.
