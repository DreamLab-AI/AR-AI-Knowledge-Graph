/**
 * GroupSection localValues wiring (defect-1 regression).
 *
 * The critical wiring bug: SettingsPanel rendered <GroupSection group={...} />
 * WITHOUT localValues/onLocalChange, so every transient localKey field was dead —
 * the method select snapped to a placeholder and its showWhen-dependent sliders
 * never rendered. These tests assert GroupSection honours localValues for showWhen
 * and forwards onLocalChange, and that the Run Grouping action POSTs the CURRENT
 * localValues (not hardcoded defaults). A green version of this test in WP2 would
 * have caught the missing props.
 */

import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, cleanup, act } from '@testing-library/react';
import type { LucideIcon } from 'lucide-react';
import { GroupSection } from '../GroupSection';
import { SettingAction } from '../SettingAction';
import type { RegistryField, RegistryGroup } from '../../registry/types';

const NoopIcon: LucideIcon = (() => null) as unknown as LucideIcon;

const RUN_GROUPING_FIELDS: RegistryField[] = [
  { key: 'groupingMethod', subgroup: 'Run Grouping', label: 'Method', type: 'select', localKey: 'method', default: 'communities', options: ['communities', 'kmeans', 'dbscan'] },
  { key: 'groupingNumClusters', subgroup: 'Run Grouping', label: 'Cluster Count (K)', type: 'slider', localKey: 'numClusters', default: 8, min: 2, max: 50, step: 1, showWhen: { localKey: 'method', equals: 'kmeans' } },
  { key: 'groupingEps', subgroup: 'Run Grouping', label: 'Neighbourhood (eps)', type: 'slider', localKey: 'eps', default: 5, min: 0.1, max: 10, step: 0.1, showWhen: { localKey: 'method', equals: 'dbscan' } },
  { key: 'groupingResolution', subgroup: 'Run Grouping', label: 'Resolution', type: 'slider', localKey: 'resolution', default: 1, min: 0.1, max: 5, step: 0.1, showWhen: { localKey: 'method', equals: 'communities' } },
];

function groupWith(fields: RegistryField[]): RegistryGroup {
  return {
    id: 'quality',
    label: 'Filtering & Quality',
    icon: NoopIcon,
    description: '',
    hotkey: '4',
    loadPaths: [],
    fields,
  };
}

const tid = (key: string) => `setting-quality.${key}`;

beforeEach(() => cleanup());

describe('GroupSection localValues / showWhen wiring (defect-1)', () => {
  it('renders only the showWhen-matching slider for the current method (communities)', () => {
    render(
      <GroupSection
        group={groupWith(RUN_GROUPING_FIELDS)}
        localValues={{ method: 'communities', numClusters: 8, eps: 5, resolution: 1 }}
        onLocalChange={() => {}}
      />,
    );

    // method select always present
    expect(screen.getByTestId(tid('groupingMethod'))).toBeInTheDocument();
    // communities → only the resolution slider is visible
    expect(screen.getByTestId(tid('groupingResolution'))).toBeInTheDocument();
    expect(screen.queryByTestId(tid('groupingNumClusters'))).not.toBeInTheDocument();
    expect(screen.queryByTestId(tid('groupingEps'))).not.toBeInTheDocument();
  });

  it('reveals the kmeans slider (and hides the others) when method=kmeans', () => {
    render(
      <GroupSection
        group={groupWith(RUN_GROUPING_FIELDS)}
        localValues={{ method: 'kmeans', numClusters: 8, eps: 5, resolution: 1 }}
        onLocalChange={() => {}}
      />,
    );

    expect(screen.getByTestId(tid('groupingNumClusters'))).toBeInTheDocument();
    expect(screen.queryByTestId(tid('groupingResolution'))).not.toBeInTheDocument();
    expect(screen.queryByTestId(tid('groupingEps'))).not.toBeInTheDocument();
  });

  it('reveals the dbscan sliders when method=dbscan', () => {
    render(
      <GroupSection
        group={groupWith(RUN_GROUPING_FIELDS)}
        localValues={{ method: 'dbscan', numClusters: 8, eps: 5, resolution: 1 }}
        onLocalChange={() => {}}
      />,
    );

    expect(screen.getByTestId(tid('groupingEps'))).toBeInTheDocument();
    expect(screen.queryByTestId(tid('groupingNumClusters'))).not.toBeInTheDocument();
    expect(screen.queryByTestId(tid('groupingResolution'))).not.toBeInTheDocument();
  });

  it('forwards onLocalChange to the visible slider so edits flow back to the map', () => {
    const onLocalChange = vi.fn();
    render(
      <GroupSection
        group={groupWith(RUN_GROUPING_FIELDS)}
        localValues={{ method: 'kmeans', numClusters: 8, eps: 5, resolution: 1 }}
        onLocalChange={onLocalChange}
      />,
    );

    const slider = screen.getByTestId(tid('groupingNumClusters'));
    fireEvent.change(slider, { target: { value: '12' } });
    expect(onLocalChange).toHaveBeenCalledWith('numClusters', 12);
  });
});

// SettingAction must POST the CURRENT localValues, not hardcoded defaults.
// The mock must be COMPLETE: settingsApi/endpoints.ts registers axios.interceptors
// at import time, which this test transitively pulls in via the store.
vi.mock('axios', () => {
  const post = vi.fn().mockResolvedValue({ data: {} });
  const axios = {
    post,
    get: vi.fn().mockResolvedValue({ data: {} }),
    put: vi.fn().mockResolvedValue({ data: {} }),
    delete: vi.fn().mockResolvedValue({ data: {} }),
    interceptors: { request: { use: vi.fn() }, response: { use: vi.fn() } },
  };
  return { default: axios };
});

describe('SettingAction run_clustering POST body reflects localValues (defect-1)', () => {
  it('sends the selected method + its params (kmeans → numClusters)', async () => {
    const axios = (await import('axios')).default as unknown as { post: ReturnType<typeof vi.fn> };
    axios.post.mockClear();

    render(
      <SettingAction
        field={{ key: 'runGrouping', label: 'Run Grouping', type: 'action-button', action: 'run_clustering' }}
        testId="setting-quality.runGrouping"
        localValues={{ method: 'kmeans', numClusters: 12, eps: 5, minSamples: 3, resolution: 1 }}
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId('setting-quality.runGrouping'));
    });

    expect(axios.post).toHaveBeenCalledWith('/api/analytics/clustering/run', {
      method: 'kmeans',
      params: { numClusters: 12 },
    });
  });

  it('sends communities resolution from localValues (not the {resolution:1} hardcoded default)', async () => {
    const axios = (await import('axios')).default as unknown as { post: ReturnType<typeof vi.fn> };
    axios.post.mockClear();

    render(
      <SettingAction
        field={{ key: 'runGrouping', label: 'Run Grouping', type: 'action-button', action: 'run_clustering' }}
        testId="setting-quality.runGrouping"
        localValues={{ method: 'communities', resolution: 2.5 }}
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId('setting-quality.runGrouping'));
    });

    expect(axios.post).toHaveBeenCalledWith('/api/analytics/clustering/run', {
      method: 'communities',
      params: { resolution: 2.5 },
    });
  });
});
