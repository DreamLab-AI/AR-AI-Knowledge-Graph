import React, { useState, useEffect, useCallback } from 'react';
import { graphDataManager, type GraphData, type Node } from '../managers/graphDataManager';
import { useSettingsStore } from '../../../store/settingsStore';

/**
 * Assertion-version attribution (ADR-049) threaded into the node-selected event
 * by GraphManager/useGraphSelection. The panel renders it only — it never
 * fetches. `signatureValid` reflects native-envelope signature verification;
 * the agent identity is the authenticated principal (did:nostr), never a
 * client-chosen field.
 */
export interface NodeAttribution {
  didNostr: string;
  activityUrn: string;
  generatedAtTime: string;
  signatureValid: boolean;
}

export interface NodeSelectionDetail {
  nodeId: string;
  label: string;
  metadata?: Record<string, any>;
  connectionCount: number;
  neighbors: Array<{ id: string; label: string }>;
  attribution?: NodeAttribution;
}

/**
 * Slide-in panel that displays details for the currently selected graph node.
 * Listens for 'visionclaw:node-selected' custom events dispatched by GraphManager.
 */
export const NodeDetailPanel: React.FC = () => {
  const [detail, setDetail] = useState<NodeSelectionDetail | null>(null);
  const [visible, setVisible] = useState(false);
  // W-G phase-1 client-only flag (default-off). When off, the attribution
  // section renders nothing even if the payload carries attribution.
  const showAttribution = useSettingsStore(
    s => s.settings?.provenance?.showAttribution ?? false
  );

  const handleNodeSelected = useCallback((event: Event) => {
    const customEvent = event as CustomEvent<NodeSelectionDetail | null>;
    const payload = customEvent.detail;
    if (payload) {
      setDetail(payload);
      setVisible(true);
    } else {
      setVisible(false);
    }
  }, []);

  useEffect(() => {
    window.addEventListener('visionclaw:node-selected', handleNodeSelected);
    return () => {
      window.removeEventListener('visionclaw:node-selected', handleNodeSelected);
    };
  }, [handleNodeSelected]);

  const handleClose = useCallback(() => {
    setVisible(false);
    // Dispatch deselection so GraphManager clears highlight edges
    window.dispatchEvent(new CustomEvent('visionclaw:node-deselect'));
  }, []);

  const handleNeighborClick = useCallback((neighborId: string) => {
    // Dispatch a search event to fly to the neighbor and select it
    window.dispatchEvent(new CustomEvent('visionclaw:search', {
      detail: { query: '', nodeId: neighborId },
    }));
  }, []);

  const handleOpenFullPage = useCallback(() => {
    if (!detail) return;
    const meta = detail.metadata || {};
    const pageUrl = meta.page_url || meta.pageUrl || meta.url;
    if (pageUrl) {
      window.open(pageUrl, '_blank', 'noopener,noreferrer');
      return;
    }
    const filePath = meta.file_path || meta.filePath || meta.path;
    const target = filePath || detail.label;
    if (target) {
      window.open(
        `https://narrativegoldmine.com/#/page/${encodeURIComponent(target)}`,
        '_blank',
        'noopener,noreferrer'
      );
    }
  }, [detail]);

  if (!detail) return null;

  const contentPreview = extractContentPreview(detail.metadata);

  return (
    <div
      role="complementary"
      aria-label="Node details"
      style={{
        position: 'fixed',
        top: 0,
        right: visible ? 0 : -340,
        width: 320,
        height: '100vh',
        backgroundColor: 'rgba(10, 10, 30, 0.92)',
        backdropFilter: 'blur(12px)',
        borderLeft: '1px solid rgba(255, 255, 255, 0.1)',
        color: '#e0e0e0',
        fontFamily: '"Inter", "Segoe UI", sans-serif',
        fontSize: 13,
        zIndex: 1100,
        transition: 'right 0.25s ease-out',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        pointerEvents: 'auto',
      }}
    >
      {/* Header */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '16px 16px 12px',
        borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
      }}>
        <h2 style={{
          margin: 0,
          fontSize: 15,
          fontWeight: 600,
          color: '#ffffff',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          flex: 1,
          marginRight: 8,
        }}>
          {detail.label}
        </h2>
        <button
          onClick={handleClose}
          aria-label="Close node details"
          style={{
            background: 'rgba(255, 255, 255, 0.06)',
            border: '1px solid rgba(255, 255, 255, 0.12)',
            borderRadius: 4,
            color: '#aaa',
            cursor: 'pointer',
            fontSize: 16,
            width: 28,
            height: 28,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexShrink: 0,
          }}
        >
          x
        </button>
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '12px 16px' }}>
        {/* Metadata badges */}
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 12 }}>
          <Badge label="Connections" value={String(detail.connectionCount)} />
          {detail.metadata?.type && (
            <Badge label="Type" value={detail.metadata.type} />
          )}
          {detail.metadata?.domain && (
            <Badge label="Domain" value={detail.metadata.domain} />
          )}
        </div>

        {/* Attribution (ADR-049 assertion-version provenance) */}
        {showAttribution && detail.attribution && (
          <div style={{ marginBottom: 14 }}>
            <h3 style={{
              margin: '0 0 8px',
              fontSize: 12,
              fontWeight: 600,
              textTransform: 'uppercase',
              letterSpacing: 0.5,
              color: '#888',
            }}>
              Attribution
            </h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <Badge label="Agent" value={truncateMiddle(detail.attribution.didNostr, 28)} />
              <Badge label="Activity" value={truncateMiddle(detail.attribution.activityUrn, 28)} />
              <Badge
                label="Generated"
                value={formatTimestamp(detail.attribution.generatedAtTime)}
              />
              <span style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 6,
                alignSelf: 'flex-start',
                padding: '3px 10px',
                borderRadius: 999,
                fontSize: 11,
                fontWeight: 600,
                border: `1px solid ${detail.attribution.signatureValid ? 'rgba(78, 205, 120, 0.5)' : 'rgba(230, 180, 60, 0.5)'}`,
                backgroundColor: detail.attribution.signatureValid ? 'rgba(78, 205, 120, 0.15)' : 'rgba(230, 180, 60, 0.15)',
                color: detail.attribution.signatureValid ? '#6fdca0' : '#e6c24a',
              }}>
                {detail.attribution.signatureValid ? 'Signature verified' : 'Signature unverified'}
              </span>
            </div>
          </div>
        )}

        {/* Content preview */}
        {contentPreview && (
          <div style={{
            backgroundColor: 'rgba(255, 255, 255, 0.04)',
            borderRadius: 6,
            padding: '10px 12px',
            marginBottom: 14,
            lineHeight: 1.5,
            fontSize: 12,
            color: '#c0c0c0',
          }}>
            {contentPreview}
          </div>
        )}

        {/* Neighbors */}
        {detail.neighbors.length > 0 && (
          <div>
            <h3 style={{
              margin: '0 0 8px',
              fontSize: 12,
              fontWeight: 600,
              textTransform: 'uppercase',
              letterSpacing: 0.5,
              color: '#888',
            }}>
              Connected Nodes ({detail.neighbors.length})
            </h3>
            <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
              {detail.neighbors.slice(0, 30).map(n => (
                <li key={n.id}>
                  <button
                    onClick={() => handleNeighborClick(n.id)}
                    style={{
                      display: 'block',
                      width: '100%',
                      textAlign: 'left',
                      background: 'transparent',
                      border: 'none',
                      borderBottom: '1px solid rgba(255, 255, 255, 0.04)',
                      color: '#8ecfff',
                      cursor: 'pointer',
                      padding: '6px 4px',
                      fontSize: 12,
                      fontFamily: 'inherit',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {n.label}
                  </button>
                </li>
              ))}
              {detail.neighbors.length > 30 && (
                <li style={{ padding: '6px 4px', color: '#666', fontSize: 11 }}>
                  ...and {detail.neighbors.length - 30} more
                </li>
              )}
            </ul>
          </div>
        )}
      </div>

      {/* Footer: Open full page */}
      <div style={{
        padding: '12px 16px',
        borderTop: '1px solid rgba(255, 255, 255, 0.08)',
      }}>
        <button
          onClick={handleOpenFullPage}
          style={{
            width: '100%',
            padding: '8px 12px',
            backgroundColor: 'rgba(78, 205, 196, 0.15)',
            border: '1px solid rgba(78, 205, 196, 0.3)',
            borderRadius: 6,
            color: '#4ECDC4',
            cursor: 'pointer',
            fontSize: 13,
            fontFamily: 'inherit',
            fontWeight: 500,
          }}
        >
          Open full page
        </button>
      </div>
    </div>
  );
};

const Badge: React.FC<{ label: string; value: string }> = ({ label, value }) => (
  <span style={{
    display: 'inline-flex',
    alignItems: 'center',
    gap: 4,
    padding: '3px 8px',
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderRadius: 4,
    fontSize: 11,
    color: '#bbb',
  }}>
    <span style={{ color: '#777' }}>{label}:</span>
    <span style={{ color: '#ddd' }}>{value}</span>
  </span>
);

/** Middle-ellipsis a long mono identifier (did / URN) to `max` chars. */
function truncateMiddle(value: string, max: number): string {
  if (!value || value.length <= max) return value;
  const keep = Math.max(4, Math.floor((max - 1) / 2));
  return `${value.slice(0, keep)}…${value.slice(-keep)}`;
}

/** Render an ISO prov:generatedAtTime as a locale string; fall back to raw. */
function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function extractContentPreview(metadata?: Record<string, any>): string | null {
  if (!metadata) return null;
  const content = metadata.content || metadata.description || metadata.summary
    || metadata.body || metadata.text || metadata.excerpt;
  if (!content || typeof content !== 'string') return null;
  return content.length > 300 ? content.slice(0, 300) + '...' : content;
}

export default NodeDetailPanel;
