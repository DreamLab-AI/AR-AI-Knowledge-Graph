/**
 * GlassPanel elevation discipline (design-spec.md §5.1).
 *
 * Asserts the primitive maps its `elevation` prop to the right cc-glass modifier
 * class, so the glass-on-glass blur treatment is driven from ONE place rather
 * than per-surface hacks:
 *  - base (default) → cc-glass only,
 *  - overlay        → cc-glass + cc-glass--overlay (stronger backdrop blur/tint),
 *  - inset          → cc-glass + cc-glass--inset (drops the redundant re-blur),
 *  - overlay + accent compose (the SettingsPanel case).
 */

import React from 'react';
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { GlassPanel } from '../GlassPanel';

describe('GlassPanel elevation', () => {
  it('defaults to the base tier: cc-glass with neither overlay nor inset', () => {
    render(<GlassPanel data-testid="gp-base">x</GlassPanel>);
    const el = screen.getByTestId('gp-base');
    expect(el).toHaveClass('cc-glass');
    expect(el).not.toHaveClass('cc-glass--overlay');
    expect(el).not.toHaveClass('cc-glass--inset');
  });

  it('elevation="overlay" adds the stronger-blur overlay modifier', () => {
    render(<GlassPanel elevation="overlay" data-testid="gp-overlay">x</GlassPanel>);
    const el = screen.getByTestId('gp-overlay');
    expect(el).toHaveClass('cc-glass');
    expect(el).toHaveClass('cc-glass--overlay');
    expect(el).not.toHaveClass('cc-glass--inset');
  });

  it('elevation="inset" adds the flat inset modifier and no overlay', () => {
    render(<GlassPanel elevation="inset" data-testid="gp-inset">x</GlassPanel>);
    const el = screen.getByTestId('gp-inset');
    expect(el).toHaveClass('cc-glass');
    expect(el).toHaveClass('cc-glass--inset');
    expect(el).not.toHaveClass('cc-glass--overlay');
  });

  it('composes overlay with the accent ring (the SettingsPanel treatment)', () => {
    render(
      <GlassPanel elevation="overlay" accent data-testid="gp-accent-overlay">x</GlassPanel>,
    );
    const el = screen.getByTestId('gp-accent-overlay');
    expect(el).toHaveClass('cc-glass');
    expect(el).toHaveClass('cc-glass--overlay');
    expect(el).toHaveClass('cc-glass--accent');
  });

  it('preserves caller className alongside the elevation modifier', () => {
    render(
      <GlassPanel elevation="overlay" className="w-72 p-3" data-testid="gp-merge">x</GlassPanel>,
    );
    const el = screen.getByTestId('gp-merge');
    expect(el).toHaveClass('cc-glass--overlay', 'w-72', 'p-3');
  });
});
