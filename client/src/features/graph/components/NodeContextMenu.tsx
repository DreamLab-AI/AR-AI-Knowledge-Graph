import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  fetchNodeRelations,
  expandNode,
  type RelationCount,
  type ExpandDirection,
} from '../../../api/graphExpandApi';
import { graphDataManager, type Node, type Edge } from '../managers/graphDataManager';
import { createLogger, createErrorMetadata } from '../../../utils/loggerConfig';

const logger = createLogger('NodeContextMenu');

/** Payload of the visionclaw:node-contextmenu event dispatched by GraphManager. */
interface ContextMenuDetail {
  nodeId: string;
  label: string;
  x: number;
  y: number;
  /** Live worker/SAB world position of the node, seeded into the merge. */
  position?: { x: number; y: number; z: number } | null;
}

interface MenuItem {
  edgeType: string;
  label: string;
  count: number;
  direction: ExpandDirection;
}

const EXPAND_LIMIT = 25;

/**
 * Right-click additive-expansion menu (Graph2VR desktop migration).
 *
 * Flow: node right-click → GraphManager dispatches `visionclaw:node-contextmenu`
 * → this menu fetches GET /relations, lists "Expand: <label> (N)" per predicate
 * and direction → clicking an item POSTs /expand and ADDITIVELY merges the
 * returned nodes/edges via graphDataManager.mergeGraphData (existing nodes keep
 * their positions; only new nodes are seeded near the anchor).
 */
export const NodeContextMenu: React.FC = () => {
  const [detail, setDetail] = useState<ContextMenuDetail | null>(null);
  const [items, setItems] = useState<MenuItem[]>([]);
  const [status, setStatus] = useState<'idle' | 'loading' | 'error' | 'ready'>('idle');
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  // Monotonic token: every open bumps it; a relations response is applied only
  // if its token still matches. Prevents node A's predicates landing in node B's
  // menu when the user right-clicks B before A's fetch resolves.
  const requestIdRef = useRef(0);

  const close = useCallback(() => {
    requestIdRef.current++; // invalidate any in-flight relations fetch
    setDetail(null);
    setItems([]);
    setStatus('idle');
    setBusyKey(null);
    setNotice(null);
  }, []);

  // Open on the context-menu event; fetch the relation summary.
  useEffect(() => {
    const handler = (event: Event) => {
      const d = (event as CustomEvent<ContextMenuDetail>).detail;
      if (!d) return;
      const token = ++requestIdRef.current;
      setDetail(d);
      setItems([]);
      setNotice(null);
      setStatus('loading');

      fetchNodeRelations(d.nodeId)
        .then(rel => {
          if (token !== requestIdRef.current) return; // stale — a newer menu opened
          const toItems = (arr: RelationCount[], direction: ExpandDirection): MenuItem[] =>
            arr.map(r => ({ edgeType: r.edgeType, label: r.label, count: r.count, direction }));
          const merged = [
            ...toItems(rel.outgoing, 'outgoing'),
            ...toItems(rel.incoming, 'incoming'),
          ].filter(i => i.count > 0);
          setItems(merged);
          setStatus('ready');
        })
        .catch(err => {
          if (token !== requestIdRef.current) return;
          logger.error('fetchNodeRelations failed:', createErrorMetadata(err));
          setStatus('error');
        });
    };
    window.addEventListener('visionclaw:node-contextmenu', handler);
    return () => window.removeEventListener('visionclaw:node-contextmenu', handler);
  }, []);

  // Dismiss on outside click / Escape.
  useEffect(() => {
    if (!detail) return;
    const onDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as HTMLElement)) close();
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    // Defer to skip the opening right-click's own mousedown.
    const id = window.setTimeout(() => window.addEventListener('mousedown', onDown), 0);
    window.addEventListener('keydown', onKey);
    return () => {
      window.clearTimeout(id);
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('keydown', onKey);
    };
  }, [detail, close]);

  const handleExpand = useCallback(async (item: MenuItem) => {
    if (!detail) return;
    const key = `${item.direction}:${item.edgeType}`;
    setBusyKey(key);
    setNotice(null);
    try {
      const res = await expandNode(detail.nodeId, {
        edgeType: item.edgeType,
        direction: item.direction,
        limit: EXPAND_LIMIT,
      });

      // Map the expansion payload (numeric ids) onto the string-keyed topology.
      const nodes: Node[] = res.nodes.map(n => ({
        id: String(n.id),
        label: n.label,
        position: { x: 0, y: 0, z: 0 }, // seeded near the anchor by mergeGraphData
        metadata: {
          metadataId: n.metadataId,
          ...(n.nodeType ? { type: n.nodeType } : {}),
        } as Node['metadata'],
      }));
      const edges: Edge[] = res.edges.map(e => ({
        id: `${e.source}-${e.target}-${e.edgeType}`,
        source: String(e.source),
        target: String(e.target),
        label: e.edgeType,
        edgeType: e.edgeType,
        weight: e.weight,
      }));

      const { nodesAdded, edgesAdded } = await graphDataManager.mergeGraphData(
        nodes, edges, detail.nodeId, detail.position ?? undefined,
      );

      if (nodesAdded === 0 && edgesAdded === 0) {
        setNotice('Already expanded — no new nodes');
        setBusyKey(null);
      } else {
        close();
      }
    } catch (err) {
      logger.error('expand/merge failed:', createErrorMetadata(err));
      setNotice('Expansion failed');
      setBusyKey(null);
    }
  }, [detail, close]);

  if (!detail) return null;

  // Clamp position so the menu stays on-screen.
  const MENU_W = 260;
  const left = Math.min(detail.x, window.innerWidth - MENU_W - 8);
  const top = Math.min(detail.y, window.innerHeight - 320);

  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label={`Expand ${detail.label}`}
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: 'fixed',
        left: Math.max(8, left),
        top: Math.max(8, top),
        width: MENU_W,
        maxHeight: 320,
        overflowY: 'auto',
        backgroundColor: 'rgba(10, 10, 30, 0.96)',
        backdropFilter: 'blur(12px)',
        border: '1px solid rgba(255, 255, 255, 0.12)',
        borderRadius: 8,
        boxShadow: '0 8px 32px rgba(0,0,0,0.5)',
        color: '#e0e0e0',
        fontFamily: '"Inter", "Segoe UI", sans-serif',
        fontSize: 13,
        zIndex: 1200,
        padding: '6px 0',
        pointerEvents: 'auto',
      }}
    >
      <div style={{
        padding: '4px 12px 8px',
        borderBottom: '1px solid rgba(255,255,255,0.08)',
        color: '#8ecfff',
        fontWeight: 600,
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
      }}>
        {detail.label}
      </div>

      {status === 'loading' && (
        <div style={{ padding: '10px 12px', color: '#888' }}>Loading relations…</div>
      )}
      {status === 'error' && (
        <div style={{ padding: '10px 12px', color: '#e6a24a' }}>Failed to load relations</div>
      )}
      {status === 'ready' && items.length === 0 && (
        <div style={{ padding: '10px 12px', color: '#888' }}>No relations to expand</div>
      )}

      {status === 'ready' && items.map(item => {
        const key = `${item.direction}:${item.edgeType}`;
        const busy = busyKey === key;
        const arrow = item.direction === 'outgoing' ? '→' : '←';
        return (
          <button
            key={key}
            role="menuitem"
            disabled={busyKey !== null}
            onClick={() => handleExpand(item)}
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: 8,
              width: '100%',
              textAlign: 'left',
              background: busy ? 'rgba(78, 205, 196, 0.15)' : 'transparent',
              border: 'none',
              color: busyKey !== null && !busy ? '#666' : '#ddd',
              cursor: busyKey !== null ? 'default' : 'pointer',
              padding: '7px 12px',
              fontSize: 12.5,
              fontFamily: 'inherit',
            }}
          >
            <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              <span style={{ color: '#6fdca0', marginRight: 6 }}>{arrow}</span>
              Expand: {item.label}
            </span>
            <span style={{ color: '#777', flexShrink: 0 }}>
              {busy ? '…' : `(${item.count})`}
            </span>
          </button>
        );
      })}

      {notice && (
        <div style={{ padding: '8px 12px', color: '#e6c24a', fontSize: 11.5, borderTop: '1px solid rgba(255,255,255,0.08)' }}>
          {notice}
        </div>
      )}
    </div>
  );
};

export default NodeContextMenu;
