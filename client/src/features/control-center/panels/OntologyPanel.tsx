/**
 * OntologyPanel — glass wrapper around the existing OntologyTabContent, kept
 * inside an ErrorBoundary exactly as the legacy IntegratedControlPanel did.
 * design-spec.md §1.10, §2. The bespoke ontology state is unchanged.
 */

import React from 'react';
import { GlassPanel } from '../primitives/GlassPanel';
import ErrorBoundary from '../../../components/ErrorBoundary';
import { OntologyTabContent } from '../../ontology/components/OntologyTabContent';

export const OntologyPanel: React.FC = () => (
  <GlassPanel
    role="region"
    aria-label="Ontology"
    data-testid="panel-ontology-body"
    className="p-3 overflow-y-auto max-h-full"
  >
    <ErrorBoundary>
      <OntologyTabContent />
    </ErrorBoundary>
  </GlassPanel>
);

OntologyPanel.displayName = 'OntologyPanel';

export default OntologyPanel;
