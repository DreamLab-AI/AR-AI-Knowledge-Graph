/**
 * SolidPanel — glass wrapper around the existing SolidTabContent.
 * design-spec.md §1.10, §2. The bespoke Solid Pod state is unchanged; we only
 * re-host it inside the new shell.
 */

import React from 'react';
import { GlassPanel } from '../primitives/GlassPanel';
import { SolidTabContent } from '../../solid/components/SolidTabContent';

export const SolidPanel: React.FC = () => (
  <GlassPanel
    role="region"
    aria-label="Solid Pod"
    data-testid="panel-solid-body"
    className="p-3 overflow-y-auto max-h-full"
  >
    <SolidTabContent />
  </GlassPanel>
);

SolidPanel.displayName = 'SolidPanel';

export default SolidPanel;
