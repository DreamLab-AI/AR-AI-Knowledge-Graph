// REC-4 / ADR-043 resurrection (PRD-023 WP-8): the four-KPI panel logic.
//
// Verifies the honesty rule: a computed KPI carries its evidenced value +
// confidence; an awaiting KPI renders "awaiting data source" with its source
// NAMED and NO numeric value.

import { describe, it, expect } from 'vitest';
import {
  extractKpiSummary,
  formatKpiValue,
  normaliseKpiSummary,
  normaliseKpiTile,
  type RawKpiTile,
} from '../kpiSummary';

const computedAr: RawKpiTile = {
  kpi: 'augmentation_ratio',
  label: 'Augmentation Ratio',
  status: 'computed',
  value: 3.5,
  confidence: 0.4,
  unit: 'ratio',
  numerator: 42,
  denominator: 12,
  sample_count: 54,
  snapshot_id: 7,
  window_days: 30,
  source: 'agent-action volume ÷ ACSP escalation volume',
};

const awaitingMesh: RawKpiTile = {
  kpi: 'mesh_velocity',
  label: 'Mesh Velocity',
  status: 'awaiting_data_source',
  source: 'REC-10 insight-loop timestamps',
};

describe('kpiSummary (WP-8 panel logic)', () => {
  it('unwraps a StandardResponse envelope and a bare summary alike', () => {
    const bare = { tiles: [computedAr], computed_at_ms: 1, window_days: 30 };
    expect(extractKpiSummary(bare).tiles).toHaveLength(1);

    const wrapped = { success: true, data: bare, error: null };
    expect(extractKpiSummary(wrapped).tiles).toHaveLength(1);

    // Junk degrades to an empty summary, not a throw.
    expect(extractKpiSummary(null).tiles).toBeUndefined();
    expect(normaliseKpiSummary(null)).toEqual([]);
  });

  it('formats a ratio KPI with × and an index KPI to 2dp', () => {
    expect(formatKpiValue('augmentation_ratio', 3.5, 'ratio')).toBe('3.50×');
    expect(formatKpiValue('trust_variance', 0.6234, 'index')).toBe('0.62');
  });

  it('renders a computed tile with its value, confidence and derivation', () => {
    const view = normaliseKpiTile(computedAr);
    expect(view.computed).toBe(true);
    expect(view.valueText).toBe('3.50×');
    expect(view.confidencePct).toBe(40);
    expect(view.detail).toBe('42 ÷ 12');
    expect(view.snapshotId).toBe(7);
    // A computed tile never carries the awaiting label.
    expect(view.awaitingText).toBeUndefined();
  });

  it('renders an awaiting tile as "awaiting data source" with the source named and NO value', () => {
    const view = normaliseKpiTile(awaitingMesh);
    expect(view.computed).toBe(false);
    expect(view.awaitingText).toBe('awaiting data source');
    expect(view.source).toBe('REC-10 insight-loop timestamps');
    // The honesty rule: no fabricated number on an awaiting tile.
    expect(view.valueText).toBeUndefined();
    expect(view.confidencePct).toBeUndefined();
  });

  it('degrades a computed status with no value to awaiting rather than showing a blank', () => {
    const view = normaliseKpiTile({
      kpi: 'trust_variance',
      label: 'Trust Variance',
      status: 'computed',
      source: 'Gini-Simpson dispersion',
      // value intentionally absent
    });
    expect(view.computed).toBe(false);
    expect(view.awaitingText).toBe('awaiting data source');
    expect(view.valueText).toBeUndefined();
  });

  it('maps a four-tile summary to two computed and two awaiting tiles', () => {
    const summary = {
      tiles: [
        computedAr,
        {
          kpi: 'trust_variance',
          label: 'Trust Variance',
          status: 'computed',
          value: 0.62,
          confidence: 0.3,
          unit: 'index',
          sample_count: 12,
          snapshot_id: 8,
          window_days: 30,
          source: 'Gini-Simpson dispersion of enrichment_decisions outcomes',
        } as RawKpiTile,
        awaitingMesh,
        {
          kpi: 'hitl_precision',
          label: 'HITL Precision',
          status: 'awaiting_data_source',
          source: 'broker decision outcomes surfaced by WP-4',
        } as RawKpiTile,
      ],
    };
    const views = normaliseKpiSummary(summary);
    expect(views).toHaveLength(4);
    expect(views.filter((v) => v.computed)).toHaveLength(2);
    expect(views.filter((v) => !v.computed)).toHaveLength(2);
    // Every awaiting tile still names its source.
    for (const v of views.filter((v) => !v.computed)) {
      expect(v.source.length).toBeGreaterThan(0);
      expect(v.valueText).toBeUndefined();
    }
  });
});
