# Soundings Regions-Map — Market-Value Estimate

**Analyst:** Market-value analyst, Soundings QE fleet
**Date:** 2026-07-13
**Target:** https://soundings-23l.pages.dev/regions-map
**Product:** Interactive UK-wide map of 745,485 Companies House-registered creative businesses, classified by a proprietary "Soundings operational taxonomy" over UK SIC 2007, geocoded to postcode centroids, with 60+ region drill-downs and per-company Companies House deep links. Consultancy evidence-base product (regions "added as asked"), not yet a SaaS. Multi-region sibling of a curated North West flagship map.

---

## Headline numbers (read this first)

| Lens | Basis | Range (GBP) | Central |
|------|-------|-------------|---------|
| **A — Replacement / build cost** | 40–90 person-days × £600–900/day senior UK contractor | **£24k – £81k** | ~£49k |
| **B — Market comparable (annualized)** | Licence value benchmarked to The Data City tiers + per-region commission capacity | **£20k – £40k/yr recurring** (capitalised ~£40k – £160k) | ~£30k/yr |
| **C — Strategic / IP asset value** | Taxonomy IP + reproducible pipeline + whole-UK coverage sold as an asset | **£60k – £250k** | ~£130k |

### Single combined headline range

> **£45,000 – £180,000**, most-likely **~£85,000 – £110,000** as a standalone transferable asset today.
> **Confidence: moderate.** The floor is well-anchored (rebuild cost); the ceiling is soft and depends on buyer type and whether the sale bundles the NW curated layer + brand + client relationships.

### Two strongest comparables (with prices)

1. **The Data City** — the direct functional analogue: classifies 9M+ UK companies into 500+ emerging sectors over SIC, in real time. **Published pricing: Solo £10,000/yr (1 seat), Team £22,000/yr (5 seats), Department £35,000/yr (15 seats); bespoke sector-classification build ("RTIC sponsorship") £20,000–£25,000/yr; Cluster Analysis projects from £20,000.** Source: https://thedatacity.com/pricing/
2. **Beauhurst** — premium UK private-company data platform (company + funding + people data), sold to universities, corporates and public sector (incl. G-Cloud). **Custom quote, widely reported at £10,000+/seat/yr and repeatedly flagged by reviewers as "pricey."** Sources: https://www.beauhurst.com/pricing/ , https://www.capterra.co.uk/software/1051875/beauhurst

---

## Lens A — Replacement / build cost

**Method:** code-review agent estimate of ~40–90 person-days to rebuild the functional product (single-file vanilla-JS deck.gl app + per-region static data build + geocoding/classification pipeline + taxonomy encoding), priced at UK senior contractor day rates of £600–900.

| | Days | Rate | Total |
|-|------|------|-------|
| Floor | 40 | £600 | **£24,000** |
| Central | 65 | £750 | **£48,750** |
| Ceiling | 90 | £900 | **£81,000** |

**Headline: £24k – £81k, central ~£49k.**

**Caveats:**
- This lens prices *labour to recreate what exists*, not the accumulated sector expertise embedded in the taxonomy. The taxonomy ("operational taxonomy over SIC 2007") is the one genuinely non-trivial IP element; a naïve rebuild reproduces the *structure* in hours but not the *sector-mapping judgement*, which took domain knowledge to derive.
- It is also the *hardest floor* on value: because the underlying data is free and open (Companies House bulk register + postcodes.io, both OGL), a competent contractor can reproduce the whole dataset. No rational buyer pays much above rebuild cost for the code + data alone — the premium above this floor is entirely time-to-market and taxonomy IP.

---

## Lens B — Market comparable (what a consultancy could charge)

Two monetisation modes, both benchmarked to real prices.

### B1. Productised annual licence (SaaS-like)
Benchmark: **The Data City** — the closest analogue by function (SIC-based sector classification of the whole UK company register). Its tiers: Team £22k/yr (5 seats), Department £35k/yr (15 seats), plus bespoke sector builds at £20–25k/yr (https://thedatacity.com/pricing/).

Soundings is **narrower** than The Data City (single vertical — creative; no jobs/skills overlay; no revenue/employment estimates; static rather than real-time). Discounting the Data City Team/Department tiers ~30–40% for that narrower scope gives a defensible annual licence of **£15k–£30k/yr to a single anchor client** (a combined authority, Arts Council England, Creative UK, or a university research group). Two–three anchor clients → **£30k–£70k/yr aggregate ARR potential**.

### B2. Per-region commission engine (services)
Regional creative-economy mapping is an established commissioning line. The Audience Agency + The Fifth Sector's Tees Valley Combined Authority creative-economy mapping was extensive enough to underpin a **£20m** sector investment programme (https://theaudienceagency.org/en/project/tees-valley-combined-authority-creative-economy-mapping). Combined authorities now hold real budgets for this: the **£150m Creative Places Growth Fund** was allocated in **£25m** tranches to six combined authorities in 2025 (https://www.gov.uk/government/news/regions-set-to-benefit-from-new-creative-industries-funding).

Regional mapping commissions of this kind typically run **£20k–£80k per region** (light data-map at the bottom, Tees Valley-scale ecosystem mapping at the top). Soundings drives the **marginal cost of adding a region toward zero** (dated per-region builds prove the pipeline is reproducible), so each commission is high-margin. At 3–6 commissions/yr → **£60k–£300k gross services revenue enabled**.

### Capitalised asset value from this lens
Taking a realistic sustained ARR of ~£20k–£40k and a 2–4× multiple (small, single-vertical, replicable data product): **£40k–£160k**.

**Headline: ~£20k–£40k/yr recurring; capitalised ~£40k–£160k.**

---

## Lens C — Strategic / IP asset value (sold as an asset)

**The asset = taxonomy IP + reproducible pipeline + whole-UK coverage already built** (745,485 companies, 60+ regions live, near-zero marginal cost per new region).

**Value drivers (push up):**
- Whole-UK coverage is *already built* — a buyer skips 3–6 months of build and data engineering.
- Reproducible, dated pipeline (July 2026 builds) — the pipeline itself is a transferable asset, not a one-off deliverable.
- Taxonomy as IP — the operational sector mapping over SIC 2007 is the defensible core.
- Sibling NW flagship with curated venues/festivals/institutions layer signals a productisation path and an upsell tier.

**Value drainers (push down):**
- **Free-data provenance** — Companies House + postcodes.io are OGL and fully replicable; the moat is the taxonomy + build time, not the data. This caps the strategic premium hard.
- **Registered-office vs trading-address** — surfaced in-product as "claimed, not verified"; honest, but means the geography can't support hard economic-impact claims without cleaning.
- **No revenue/employment estimates** — only an accounts-category size proxy. This is the single biggest gap vs The Data City and Beauhurst, both of which attach financials.

**Valuation:** as an IP + pipeline acqui-asset, pre-revenue but time-saving, small data products transact at ~1.5–4× build cost → **£60k–£200k**. Upper case — bundled with the NW curated layer, brand, and a book of consultancy relationships, sold to a body wanting *instant* UK creative coverage (a Creative UK-type organisation, or a Data City competitor entering the creative vertical) — up to **~£250k**, discounted throughout by replicability.

**Headline: £60k – £250k, central ~£130k.**

---

## Combined headline & synthesis

- **Floor (~£45k):** rebuild cost (£24–81k) plus a modest premium for the pre-built UK dataset and taxonomy that save 3–6 months of time-to-market. No buyer pays less than "rebuild + small time premium" because the data is free.
- **Ceiling (~£180k):** strategic IP + pipeline + whole-UK-coverage value, suppressed by free-data replicability and the registered-office / no-financials limitations. Only reaches the ~£250k strategic top if sold *with* the NW curated layer, brand and client relationships.
- **Most-likely (~£85k–£110k):** the asset is worth meaningfully more than its code (taxonomy IP + instant UK coverage) but well short of a real platform (no revenue/employment data, static, replicable source).

> ### Defensible single headline range: **£45,000 – £180,000** (central **~£85,000–£110,000**). Confidence: **moderate**.

**Cross-check against scale:** The Data City is a VC-backed platform (9M+ companies, real-time SIC, jobs/skills overlays, revenue data) valued in the £millions. Soundings is a single-vertical, single-developer artefact with static data and no financials — correctly a *small fraction* of a Data City-scale valuation, consistent with a £45k–£180k asset rather than £millions. Conversely, a single Tees Valley-scale commission underpinned a £20m programme; Soundings as the engine that produces such deliverables at near-zero marginal cost has revenue *capacity* well above £180k over a few years — but that is earned services revenue requiring a consultancy to sell it, not intrinsic asset value, so it is kept out of the headline.

---

## Sensitivity factors (what moves the number)

1. **Add revenue/employment estimates** (from accounts-category proxy → modelled turnover/headcount). Biggest single uplift — closes the core gap vs The Data City/Beauhurst. Estimated **+50–100%** to strategic value.
2. **Recurring revenue.** Currently £0. One anchor client at £20–35k/yr capitalises to the **£100k–£300k** band on a 3–4× ARR multiple. This is the fastest path above the headline ceiling.
3. **Trading-address resolution.** Cleaning registered-office → trading location would make the geography defensible for economic-impact claims and unlock combined-authority procurement at higher confidence.
4. **Buyer identity.** A Data City-type incumbent pays little (can replicate the free data). A Creative UK / combined-authority / regional screen-agency buyer wanting *instant* creative coverage pays toward the top.
5. **Bundle scope.** Selling the multi-region map alone ≈ floor–mid; bundling the NW curated venues/festivals/institutions layer + brand + client book ≈ ceiling.
6. **Free-data provenance is the structural cap.** As long as the source is Companies House + postcodes.io, the taxonomy and time-to-market are the only durable moat — this is why the ceiling stays under ~£250k.

---

## Sources

- The Data City — Pricing & Plans (Solo £10k / Team £22k / Department £35k; RTIC sponsorship £20–25k/yr; Cluster Analysis from £20k): https://thedatacity.com/pricing/
- The Data City — RTIC real-time sector classifications: https://thedatacity.com/product-service/rtics/
- The Data City — Local Government solution: https://thedatacity.com/local-government/
- Beauhurst — Pricing (custom quote): https://www.beauhurst.com/pricing/
- Beauhurst — Capterra pricing & reviews (flagged as high cost): https://www.capterra.co.uk/software/1051875/beauhurst
- Beauhurst — G-Cloud / Digital Marketplace public-sector listing: https://www.applytosupply.digitalmarketplace.service.gov.uk/g-cloud/services/683445940907143
- Glass.AI — evidence-led company/sector mapping for UK government (comparable methodology; AI-sector baseline, Creative Nation): https://www.glass.ai/
- The Audience Agency — Tees Valley Combined Authority creative-economy mapping (underpinned £20m programme): https://theaudienceagency.org/en/project/tees-valley-combined-authority-creative-economy-mapping
- The Fifth Sector — creative-economy consultancy services (Tees Valley, Leicester mapping): https://www.thefifthsector.co.uk/our-services
- GOV.UK — £150m Creative Places Growth Fund allocated in £25m tranches to combined authorities: https://www.gov.uk/government/news/regions-set-to-benefit-from-new-creative-industries-funding
- Creative Industries Policy and Evidence Centre (Creative PEC) — national creative-industries evidence body (AHRC-funded): https://www.pec.ac.uk/
- DCMS Sectors Economic Estimates — methodology (creative-industries measurement baseline): https://www.gov.uk/government/publications/dcms-sectors-economic-estimates-methodology/dcms-sector-economic-estimates-methodology
