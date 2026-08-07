/**
 * useGraphSelection — selection state, camera fly-to, and search/deselect event wiring.
 *
 * Extracted from GraphManager.tsx (Phase B1 modularisation).
 */
import { useState, useEffect, useRef } from 'react'
import * as THREE from 'three'
import type { GraphData, Node as GraphNode } from '../managers/graphDataManager'

export interface GraphSelectionOptions {
  graphData: GraphData
  nodeIdToIndexMap: Map<string, number>
  nodePositionsRef: React.MutableRefObject<Float32Array | null>
  connectionCountMap: Map<string, number>
  camera: THREE.Camera
}

export interface GraphSelectionReturn {
  selectedNodeId: string | null
  setSelectedNodeId: React.Dispatch<React.SetStateAction<string | null>>
  flyToTargetRef: React.MutableRefObject<THREE.Vector3 | null>
  flyToProgressRef: React.MutableRefObject<number>
}

/**
 * Extract ADR-049 assertion-version attribution from node metadata, tolerating
 * both camelCase and the backend snake_case. Returns undefined unless the core
 * identity fields (agent did + activity URN) are present, so provenance-less
 * nodes dispatch no attribution. The agent identity is the authenticated
 * principal recorded server-side; the client never synthesises it.
 */
function extractAttribution(
  metadata: Record<string, any> | undefined
): { didNostr: string; activityUrn: string; generatedAtTime: string; signatureValid: boolean } | undefined {
  if (!metadata) return undefined
  const didNostr = metadata.didNostr ?? metadata.did_nostr ?? metadata.agentDid ?? metadata.agent_did
  const activityUrn = metadata.activityUrn ?? metadata.activity_urn
  if (typeof didNostr !== 'string' || typeof activityUrn !== 'string') return undefined
  const generatedAtTime =
    metadata.generatedAtTime ?? metadata.generated_at_time ?? metadata.generatedAt ?? ''
  const signatureValid =
    metadata.signatureValid ?? metadata.signature_valid ?? false
  return {
    didNostr,
    activityUrn,
    generatedAtTime: String(generatedAtTime),
    signatureValid: Boolean(signatureValid),
  }
}

export function useGraphSelection(opts: GraphSelectionOptions): GraphSelectionReturn {
  const { graphData, nodeIdToIndexMap, nodePositionsRef, connectionCountMap, camera } = opts

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const flyToTargetRef  = useRef<THREE.Vector3 | null>(null)
  const flyToProgressRef = useRef(0)

  // Dispatch visionclaw:node-selected when selection changes
  useEffect(() => {
    if (!selectedNodeId) {
      window.dispatchEvent(new CustomEvent('visionclaw:node-selected', { detail: null }))
      return
    }
    const node = graphData.nodes.find(n => String(n.id) === selectedNodeId)
    if (!node) return

    const neighborIds = new Set<string>()
    graphData.edges.forEach(edge => {
      const src = String(edge.source)
      const tgt = String(edge.target)
      if (src === selectedNodeId) neighborIds.add(tgt)
      if (tgt === selectedNodeId) neighborIds.add(src)
    })
    const neighbors = Array.from(neighborIds).map(nid => {
      const n = graphData.nodes.find(nd => String(nd.id) === nid)
      return { id: nid, label: n?.label || nid }
    })

    window.dispatchEvent(new CustomEvent('visionclaw:node-selected', {
      detail: {
        nodeId: selectedNodeId,
        label: node.label,
        metadata: node.metadata || {},
        connectionCount: connectionCountMap.get(selectedNodeId) || neighborIds.size,
        neighbors,
        // ADR-049 attribution, when the node carries assertion-version provenance
        // metadata. Undefined otherwise — the panel renders the section only when
        // present AND the provenance.showAttribution flag is on.
        attribution: extractAttribution(node.metadata),
      },
    }))
  }, [selectedNodeId, graphData.nodes, graphData.edges, connectionCountMap])

  // Search and deselect event listeners
  useEffect(() => {
    const handleSearch = (event: Event) => {
      const { query, nodeId } = (event as CustomEvent).detail || {}
      let targetNode: GraphNode | undefined

      if (nodeId) {
        targetNode = graphData.nodes.find(n => String(n.id) === nodeId)
      }
      if (!targetNode && query) {
        const lq = query.toLowerCase()
        targetNode = graphData.nodes.find(n => n.label.toLowerCase().startsWith(lq))
        if (!targetNode) targetNode = graphData.nodes.find(n => n.label.toLowerCase().includes(lq))
        if (!targetNode && lq.includes(' ')) {
          const words = lq.split(/\s+/).filter((w: string) => w.length > 1)
          targetNode = graphData.nodes.find(n => {
            const label = n.label.toLowerCase()
            return words.every((w: string) => label.includes(w))
          })
        }
      }
      if (!targetNode) return

      setSelectedNodeId(String(targetNode.id))

      const idx = nodeIdToIndexMap.get(String(targetNode.id))
      const positions = nodePositionsRef.current
      let targetPos: THREE.Vector3 | null = null

      if (idx !== undefined && positions && idx * 3 + 2 < positions.length) {
        targetPos = new THREE.Vector3(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2])
      } else if (targetNode.position) {
        targetPos = new THREE.Vector3(targetNode.position.x, targetNode.position.y, targetNode.position.z)
      }

      if (targetPos) {
        const offset = new THREE.Vector3().subVectors(camera.position, targetPos).normalize().multiplyScalar(25)
        flyToTargetRef.current = targetPos.clone().add(offset)
        flyToProgressRef.current = 0
      }
    }

    const handleDeselect = () => setSelectedNodeId(null)

    window.addEventListener('visionclaw:search', handleSearch)
    window.addEventListener('visionclaw:node-deselect', handleDeselect)
    return () => {
      window.removeEventListener('visionclaw:search', handleSearch)
      window.removeEventListener('visionclaw:node-deselect', handleDeselect)
    }
  }, [graphData.nodes, nodeIdToIndexMap, camera, nodePositionsRef])

  return { selectedNodeId, setSelectedNodeId, flyToTargetRef, flyToProgressRef }
}
