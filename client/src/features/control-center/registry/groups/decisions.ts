/**
 * Group 10 — Decisions (id `decisions`, hotkey 10, 2 fields).
 *
 * Phase-1 W-G client surface for the governed decision-record subsystem
 * (ADR-047/048). Both fields are CLIENT-ONLY and DEFAULT-OFF: they gate whether
 * the graph surfaces `dl:DecisionRecord` overlays and precedent highlighting.
 * The paths carry no server bucket (serverBucketFor → null), so they persist to
 * localStorage only and never touch the settings PUT pipeline.
 *
 * Decision chains are DERIVED, bounded (max_depth) reachability with supporting
 * paths — never materialised transitive truth, never "Whelk-classified".
 */
import type { GroupData, RegistryField } from '../types';

const fields: RegistryField[] = [
  {
    key: 'showDecisionChains',
    subgroup: 'Decision Records',
    label: 'Show Decision Chains',
    type: 'toggle',
    path: 'decisions.showDecisionChains',
    default: false,
    description: 'Overlay derived, bounded decision-chain reachability (dl:caused / dl:precedentFor) with supporting paths. Reachability is query-derived, not asserted truth.',
  },
  {
    key: 'highlightPrecedents',
    subgroup: 'Decision Records',
    label: 'Highlight Precedents',
    type: 'toggle',
    path: 'decisions.highlightPrecedents',
    default: false,
    description: 'Highlight direct dl:precedentFor edges on the selected decision node.',
  },
];

export const decisions: GroupData = {
  id: 'decisions',
  label: 'Decisions',
  description: 'Client-only overlays for governed decision records and derived, bounded decision-chain reachability.',
  hotkey: '10',
  loadPaths: ['decisions'],
  fields,
};
