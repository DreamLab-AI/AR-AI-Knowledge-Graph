/**
 * SettingsPanel — the slide-out settings host. design-spec.md §2, §6.1, §6.3.
 *
 * Layout: a left rail (the 8 semantic groups + the Solid / Ontology bespoke
 * entries) and an active body. The body shows a GroupSection for a semantic
 * group, or the bespoke panel for solid/ontology. A search box filters the
 * active group's fields by label or dot-path.
 *
 * Non-modal region (§6.3): the canvas stays interactive behind it, so this is a
 * `role="region"` slide-out, not a focus trap — the palette remains the only
 * modal surface.
 */

import React, { useEffect, useMemo, useState } from 'react';
import { X } from 'lucide-react';
import { REGISTRY, GROUP_BY_ID } from '../registry/settingsRegistry';
import { PANELS } from '../registry/manifest';
import { GlassPanel } from '../primitives/GlassPanel';
import { GroupSection } from '../primitives/GroupSection';
import { SettingRow } from '../primitives/SettingRow';
import { SearchInput } from '../../design-system/components/SearchInput';
import { SolidPanel } from './SolidPanel';
import { OntologyPanel } from './OntologyPanel';
import { useControlCenterUI } from '../state/useControlCenterUI';
import { useEnsureGroupLoaded } from '../state/useEnsureGroupLoaded';
import '../styles/control-center.css';

const BESPOKE_IDS = new Set(PANELS.map((p) => p.id));

export const SettingsPanel: React.FC = () => {
  const openPanel = useControlCenterUI((s) => s.openPanel);
  const activeGroup = useControlCenterUI((s) => s.activeGroup);
  const openGroup = useControlCenterUI((s) => s.openGroup);
  const closePanel = useControlCenterUI((s) => s.closePanel);
  const ensureGroupLoaded = useEnsureGroupLoaded();

  const [query, setQuery] = useState('');

  const group = activeGroup && GROUP_BY_ID[activeGroup] ? GROUP_BY_ID[activeGroup] : null;

  // Search mode swaps GroupSection (which owns the normal-path hydration) for a
  // flat filtered list, so hydrate the active group here for that case only —
  // keeping the normal open on a single ensureLoaded via GroupSection.onFirstMount.
  useEffect(() => {
    if (query.trim() && group) ensureGroupLoaded(group);
  }, [query, group, ensureGroupLoaded]);

  const filtered = useMemo(() => {
    if (!group || !query.trim()) return [];
    const q = query.trim().toLowerCase();
    return group.fields.filter(
      (f) =>
        f.label.toLowerCase().includes(q) ||
        (f.path ?? '').toLowerCase().includes(q) ||
        (f.subgroup ?? '').toLowerCase().includes(q),
    );
  }, [group, query]);

  const renderBody = () => {
    if (activeGroup === 'solid') return <SolidPanel />;
    if (activeGroup === 'ontology') return <OntologyPanel />;
    if (!group) {
      return (
        <p className="cc-helper-text p-4 text-center">
          Pick a group from the rail, or press 1–8.
        </p>
      );
    }
    if (query.trim()) {
      return (
        <div role="region" aria-label={`${group.label} — search results`} className="flex flex-col gap-0.5">
          {filtered.length === 0 ? (
            <p className="cc-helper-text p-4 text-center">No settings match “{query}”.</p>
          ) : (
            filtered.map((f) => <SettingRow key={f.key} field={f} groupId={group.id} />)
          )}
        </div>
      );
    }
    return <GroupSection group={group} onFirstMount={ensureGroupLoaded} />;
  };

  return (
    <GlassPanel
      accent
      animated
      role="region"
      aria-label="Settings"
      aria-hidden={!openPanel}
      data-testid="settings-panel"
      data-open={openPanel}
      className="fixed left-4 top-4 bottom-24 z-40 w-[380px] max-w-[92vw] flex overflow-hidden"
      style={{
        transform: openPanel ? 'translateX(0)' : 'translateX(-112%)',
        opacity: openPanel ? 1 : 0,
        pointerEvents: openPanel ? 'auto' : 'none',
      }}
    >
      {/* Left rail */}
      <nav
        aria-label="Settings groups"
        className="shrink-0 w-14 flex flex-col items-stretch gap-1 p-1.5 border-r border-border/40 overflow-y-auto"
      >
        {REGISTRY.map((g) => {
          const Icon = g.icon;
          const isActive = activeGroup === g.id;
          return (
            <button
              key={g.id}
              type="button"
              data-testid={`group-rail-${g.id}`}
              aria-label={g.label}
              aria-current={isActive ? 'true' : undefined}
              title={g.label}
              onClick={() => openGroup(g.id)}
              className={[
                'relative flex flex-col items-center gap-0.5 py-2 rounded-md transition-colors',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                isActive ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:text-foreground hover:bg-white/5',
              ].join(' ')}
            >
              {Icon && <Icon size={16} aria-hidden="true" />}
              {g.hotkey && (
                <span className="cc-value-readout leading-none" aria-hidden="true">
                  {g.hotkey}
                </span>
              )}
            </button>
          );
        })}

        <span className="my-1 h-px bg-border/40" aria-hidden="true" />

        {PANELS.map((p) => {
          const isActive = activeGroup === p.id;
          return (
            <button
              key={p.id}
              type="button"
              data-testid={p.testid}
              aria-label={p.label}
              aria-current={isActive ? 'true' : undefined}
              title={p.label}
              onClick={() => openGroup(p.id)}
              className={[
                'flex items-center justify-center py-2 rounded-md text-[9px] uppercase tracking-wide transition-colors',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                isActive ? 'bg-primary/20 text-primary' : 'text-muted-foreground hover:text-foreground hover:bg-white/5',
              ].join(' ')}
            >
              {p.label.split(' ')[0]}
            </button>
          );
        })}
      </nav>

      {/* Body */}
      <div className="flex-1 min-w-0 flex flex-col">
        <header className="flex items-center gap-2 p-2 border-b border-border/40">
          {!BESPOKE_IDS.has(activeGroup ?? '') && (
            <div className="flex-1 min-w-0">
              <SearchInput
                value={query}
                onChange={setQuery}
                placeholder="Filter settings…"
                aria-label="Filter settings"
                className="text-xs"
              />
            </div>
          )}
          {BESPOKE_IDS.has(activeGroup ?? '') && (
            <span className="cc-title flex-1 min-w-0 truncate">
              {PANELS.find((p) => p.id === activeGroup)?.label}
            </span>
          )}
          <button
            type="button"
            data-testid="settings-panel-close"
            aria-label="Close settings"
            onClick={closePanel}
            className="shrink-0 flex items-center justify-center h-7 w-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-white/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X size={15} aria-hidden="true" />
          </button>
        </header>

        <div className="flex-1 min-h-0 overflow-y-auto p-2">
          {openPanel && renderBody()}
        </div>
      </div>
    </GlassPanel>
  );
};

SettingsPanel.displayName = 'SettingsPanel';

export default SettingsPanel;
