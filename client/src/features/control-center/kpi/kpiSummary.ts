// REC-4 / ADR-043 resurrection (PRD-023 WP-8): pure logic for the control-centre
// four-KPI dashboard panel.
//
// Renderer-free and DOM-free so the envelope unwrapping and the tile
// normalisation are unit-testable without a live fetch. The stateful consumer
// that fetches `/api/kpi/summary` is `useKpiSummary`; the panel is `KpiPanel`.
//
// The honesty rule this module enforces (WP-8 falsification): a KPI with no
// source event yet renders as "awaiting data source" with its source NAMED and
// NO numeric value. A computed KPI carries the value it was evidenced with.

export type KpiStatus = 'computed' | 'awaiting_data_source';

/** One tile as the server emits it (`services/kpi_compute.rs::KpiTile`). */
export interface RawKpiTile {
  kpi: string;
  label: string;
  status: KpiStatus | string;
  value?: number;
  confidence?: number;
  unit?: string;
  numerator?: number;
  denominator?: number;
  sample_count?: number;
  snapshot_id?: number;
  window_days?: number;
  source: string;
}

/** The `/api/kpi/summary` payload (`services/kpi_compute.rs::KpiSummary`). */
export interface RawKpiSummary {
  tiles?: RawKpiTile[];
  computed_at_ms?: number;
  window_days?: number;
  sha?: string;
}

/** A tile normalised for rendering. */
export interface KpiTileView {
  kpi: string;
  label: string;
  computed: boolean;
  /** Present only for a computed KPI — never fabricated for an awaiting one. */
  valueText?: string;
  /** Confidence as a whole percentage (0–100), computed KPIs only. */
  confidencePct?: number;
  /** A short derivation detail (e.g. numerator ÷ denominator). */
  detail?: string;
  sampleCount?: number;
  snapshotId?: number;
  /** The named source stream — always present (documents or names-what's-missing). */
  source: string;
  /** The honest label for an awaiting KPI. Absent for a computed one. */
  awaitingText?: string;
}

/**
 * Unwrap the response body. The KPI routes answer through the `StandardResponse`
 * envelope (`{ success, data, … }`), so a fetch may hand us either the envelope
 * or the bare summary. Tolerate both.
 */
export function extractKpiSummary(raw: unknown): RawKpiSummary {
  if (!raw || typeof raw !== 'object') return {};
  const obj = raw as Record<string, unknown>;
  // StandardResponse envelope → the summary lives under `.data`.
  if ('data' in obj && obj.data && typeof obj.data === 'object' && 'tiles' in (obj.data as object)) {
    return obj.data as RawKpiSummary;
  }
  return obj as RawKpiSummary;
}

/** Format a computed value for its KPI (ratio → `×`, index → 2dp). */
export function formatKpiValue(kpi: string, value: number, unit?: string): string {
  if (kpi === 'augmentation_ratio' || unit === 'ratio') {
    return `${value.toFixed(2)}×`;
  }
  // Trust Variance and any index-like KPI: a bounded 0–1 figure.
  return value.toFixed(2);
}

/**
 * Normalise one raw tile into a render model. A tile is treated as computed only
 * when its status says so AND it carries a numeric value — a status of
 * `computed` without a value degrades to awaiting rather than showing a blank
 * or a zero as if it were real.
 */
export function normaliseKpiTile(t: RawKpiTile): KpiTileView {
  const isComputed = t.status === 'computed' && typeof t.value === 'number';

  if (!isComputed) {
    return {
      kpi: t.kpi,
      label: t.label,
      computed: false,
      source: t.source,
      awaitingText: 'awaiting data source',
    };
  }

  const value = t.value as number;
  const detail =
    typeof t.numerator === 'number' && typeof t.denominator === 'number'
      ? `${t.numerator} ÷ ${t.denominator}`
      : undefined;

  return {
    kpi: t.kpi,
    label: t.label,
    computed: true,
    valueText: formatKpiValue(t.kpi, value, t.unit),
    confidencePct:
      typeof t.confidence === 'number' ? Math.round(t.confidence * 100) : undefined,
    detail,
    sampleCount: t.sample_count,
    snapshotId: t.snapshot_id,
    source: t.source,
  };
}

/** Normalise a raw response (or envelope) into the four render-ready tiles. */
export function normaliseKpiSummary(raw: unknown): KpiTileView[] {
  const summary = extractKpiSummary(raw);
  return (summary.tiles ?? []).map(normaliseKpiTile);
}
