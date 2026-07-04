/**
 * useControlCenterHotkeys — Escape scoping (defect-2).
 *
 * Pressing Escape while a Radix Select dropdown (a popper-positioned overlay) is
 * open must dismiss only the dropdown, not the whole SettingsPanel. The hook now
 * bails out of its close-panel branch when a [data-radix-popper-content-wrapper]
 * or [role="listbox"] is mounted, leaving Escape for Radix's own handler.
 */

import React from 'react';
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, fireEvent, cleanup, act } from '@testing-library/react';
import { useControlCenterHotkeys } from '../useControlCenterHotkeys';
import { useControlCenterUI } from '../../state/useControlCenterUI';

const Harness: React.FC = () => {
  useControlCenterHotkeys();
  return null;
};

beforeEach(() => {
  useControlCenterUI.setState({ openPanel: true, activeGroup: 'quality' });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  document.body.innerHTML = '';
});

describe('Escape scoping with an open popover (defect-2)', () => {
  it('does NOT close the panel when a Radix popper wrapper is open', () => {
    render(<Harness />);
    const popper = document.createElement('div');
    popper.setAttribute('data-radix-popper-content-wrapper', '');
    document.body.appendChild(popper);

    act(() => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    expect(useControlCenterUI.getState().openPanel).toBe(true);
  });

  it('does NOT close the panel when a role="listbox" is open', () => {
    render(<Harness />);
    const listbox = document.createElement('div');
    listbox.setAttribute('role', 'listbox');
    document.body.appendChild(listbox);

    act(() => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    expect(useControlCenterUI.getState().openPanel).toBe(true);
  });

  it('DOES close the panel on Escape when no popover is open', () => {
    render(<Harness />);

    act(() => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    expect(useControlCenterUI.getState().openPanel).toBe(false);
  });
});
