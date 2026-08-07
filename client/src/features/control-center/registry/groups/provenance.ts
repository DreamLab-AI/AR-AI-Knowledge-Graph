/**
 * Group 11 — Provenance (id `provenance`, hotkey 11, 2 fields).
 *
 * Phase-1 W-G client surface for the assertion-version provenance subsystem
 * (ADR-049). Both fields are CLIENT-ONLY and DEFAULT-OFF: they gate whether the
 * node detail panel renders attribution (did:nostr / activity URN /
 * prov:generatedAtTime / signature) and whether the proposal list renders the
 * per-gate status chips (conflict / Whelk consistency / ACSP).
 *
 * The Whelk chip reports classifier consistency of the asserted projection
 * (urn:ngm:graph:ontology:assert) — never reachability, never "Whelk-classified"
 * transitive truth. Paths carry no server bucket (serverBucketFor → null).
 */
import type { GroupData, RegistryField } from '../types';

const fields: RegistryField[] = [
  {
    key: 'showAttribution',
    subgroup: 'Attribution',
    label: 'Show Node Attribution',
    type: 'toggle',
    path: 'provenance.showAttribution',
    default: false,
    description: 'Show the provenance attribution section (did:nostr, activity URN, generated-at time, signature status) on the node detail panel.',
  },
  {
    key: 'showGateChips',
    subgroup: 'Governance Gates',
    label: 'Show Gate Chips',
    type: 'toggle',
    path: 'provenance.showGateChips',
    default: false,
    description: 'Show per-proposal governance gate chips (integrity conflict, Whelk asserted-projection consistency, ACSP) on the proposal list.',
  },
];

export const provenance: GroupData = {
  id: 'provenance',
  label: 'Provenance',
  description: 'Client-only overlays for assertion-version attribution and governance-gate status chips.',
  hotkey: '11',
  loadPaths: ['provenance'],
  fields,
};
