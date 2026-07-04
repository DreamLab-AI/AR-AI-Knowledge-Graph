/**
 * RendererInfo — readonly renderer capabilities display, porting the legacy
 * 'readonly' case's presentation exactly (backend dot, GPU adapter, feature
 * badges). design-spec.md §1.9.
 *
 * Sourced directly from rendering/rendererFactory's `rendererCapabilities`
 * module export rather than the settings store: the legacy field pointed
 * `path: 'rendererCapabilities'` at the settings tree, but nothing ever
 * writes that path into settings — the live value only ever lived in this
 * module-level export. Polled on a light interval since it's a mutable
 * `let`, not a reactive store slice.
 */

import React, { useEffect, useState } from 'react';
import type { RegistryField } from '../registry/types';
import { rendererCapabilities as getSnapshot } from '../../../rendering/rendererFactory';

export interface RendererInfoProps {
  field: RegistryField;
  testId: string;
}

const POLL_MS = 1000;

export const RendererInfo: React.FC<RendererInfoProps> = ({ field, testId }) => {
  const [caps, setCaps] = useState(getSnapshot);

  useEffect(() => {
    const id = setInterval(() => setCaps({ ...getSnapshot }), POLL_MS);
    return () => clearInterval(id);
  }, []);

  return (
    <div id={testId} data-testid={testId} role="status" aria-label={field.label} className="cc-helper-text flex flex-wrap items-center gap-1">
      <span
        className="inline-block h-1.5 w-1.5 rounded-full"
        style={{
          background: caps.backend === 'webgpu' ? '#10b981' : '#6b7280',
          boxShadow: caps.backend === 'webgpu' ? '0 0 4px rgba(16,185,129,0.6)' : 'none',
        }}
        aria-hidden="true"
      />
      <span>{caps.backend?.toUpperCase() ?? 'Detecting...'}</span>
      {caps.gpuAdapterName && <span className="opacity-70">— {caps.gpuAdapterName}</span>}
      {caps.tslMaterialsActive && <span className="text-violet-400">TSL Materials</span>}
      {caps.nodeBasedBloom && <span className="text-amber-400">Node Bloom</span>}
      {caps.pixelRatio && <span className="opacity-60">DPR: {caps.pixelRatio}</span>}
    </div>
  );
};

RendererInfo.displayName = 'RendererInfo';

export default RendererInfo;
