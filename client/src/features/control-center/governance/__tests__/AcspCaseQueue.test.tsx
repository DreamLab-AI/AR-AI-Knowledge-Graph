// REC-2 / D3 (PRD-023 WP-4): the case queue renders the inbox, shows the ambient
// open-case count, and decides through the WS-9 operator route.

import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

const { getDataSpy, postSpy } = vi.hoisted(() => ({
  getDataSpy: vi.fn(),
  postSpy: vi.fn(),
}));

vi.mock('../../../../services/api/UnifiedApiClient', () => ({
  unifiedApiClient: {
    getData: (...args: unknown[]) => getDataSpy(...args),
    post: (...args: unknown[]) => postSpy(...args),
  },
}));

vi.mock('../../../../store/websocketStore', () => ({
  webSocketService: {
    onMessage: () => () => {},
  },
}));

import { AcspCaseQueue } from '../AcspCaseQueue';

beforeEach(() => {
  cleanup();
  getDataSpy.mockReset();
  postSpy.mockReset();
});

describe('AcspCaseQueue (D3)', () => {
  it('renders open-case count and decides via the operator route', async () => {
    getDataSpy.mockResolvedValue({
      cases: [
        {
          id: 'case-7',
          category: 'knowledge_enrichment',
          status: 'pending',
          metadata: { target_path: 'pages/foo.md', content: 'proposed body' },
        },
      ],
      total: 1,
    });
    postSpy.mockResolvedValue({ data: { success: true } });

    render(<AcspCaseQueue />);

    // Ambient indicator shows one open case.
    const indicator = await screen.findByTestId('acsp-indicator');
    await waitFor(() => expect(indicator).toHaveTextContent('1'));

    // Expand and approve the pending case.
    fireEvent.click(indicator);
    const approve = await screen.findByText('Approve');
    fireEvent.click(approve);

    await waitFor(() => expect(postSpy).toHaveBeenCalled());
    const [url, body] = postSpy.mock.calls[0];
    expect(url).toBe('/broker/cases/case-7/decide');
    expect(body).toMatchObject({ outcome: 'approve' });
  });
});
