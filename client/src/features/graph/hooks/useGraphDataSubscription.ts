/**
 * useGraphDataSubscription — subscribes to graphDataManager and normalises incoming GraphData.
 *
 * Extracted from GraphManager.tsx (Phase B1 modularisation).
 * Handles: topology short-circuit (ADR-03 D6), initial position generation,
 * edge source/target coercion + fallback extraction, and a 5-second fallback seed.
 */
import { useEffect, useRef } from 'react'
import { graphDataManager, type GraphData } from '../managers/graphDataManager'
import { createLogger } from '../../../utils/loggerConfig'
import { debugState } from '../../../utils/clientDebugState'
import { getPositionForNode } from './useGraphNodeColors'

const logger = createLogger('useGraphDataSubscription')

export interface GraphDataSubscriptionCallbacks {
  onGraphData: (data: GraphData) => void
  onEdgePoints: (pts: number[]) => void
  onNodesAtOrigin: (v: boolean) => void
  settings: React.MutableRefObject<any>
}

export function useGraphDataSubscription(callbacks: GraphDataSubscriptionCallbacks) {
  const { onGraphData, onEdgePoints, onNodesAtOrigin, settings } = callbacks

  const lastProcessedGraphRef = useRef<GraphData | null>(null)
  const lastShapeRef = useRef<{ nodeCount: number; edgeCount: number; hash: string } | null>(null)

  useEffect(() => {
    const handleGraphUpdate = (data: GraphData): GraphData | undefined => {
      const debugSettings = settings.current?.system?.debug
      if (debugSettings?.enableNodeDebug) {
        logger.debug('Graph data updated', {
          nodeCount: data.nodes.length,
          edgeCount: data.edges.length,
          firstNode: data.nodes.length > 0 ? data.nodes[0] : null,
          hasValidData: data && Array.isArray(data.nodes) && Array.isArray(data.edges),
        })
      }
      if (debugState.isEnabled()) {
        logger.info('Graph data updated', {
          nodeCount: data.nodes.length,
          edgeCount: data.edges.length,
          firstNode: data.nodes.length > 0 ? data.nodes[0] : null,
        })
      }

      if (!data || !Array.isArray(data.nodes) || !Array.isArray(data.edges)) return undefined

      // ADR-03 D6: identity short-circuit
      if (data === lastProcessedGraphRef.current) return lastProcessedGraphRef.current ?? undefined

      const firstId = data.nodes.length > 0 ? String(data.nodes[0].id) : ''
      const lastId  = data.nodes.length > 0 ? String(data.nodes[data.nodes.length - 1].id) : ''
      const shape = {
        nodeCount: data.nodes.length,
        edgeCount: data.edges.length,
        hash: `${data.nodes.length}-${data.edges.length}-${firstId}-${lastId}`,
      }
      const prevShape = lastShapeRef.current
      if (prevShape &&
          prevShape.nodeCount === shape.nodeCount &&
          prevShape.edgeCount === shape.edgeCount &&
          prevShape.hash === shape.hash) {
        lastProcessedGraphRef.current = data
        return data
      }

      const dataWithPositions: GraphData = {
        ...data,
        nodes: data.nodes.map((node, i) => {
          const normalizedNode = typeof node.id !== 'string' ? { ...node, id: String(node.id) } : node
          if (!normalizedNode.position || (normalizedNode.position.x === 0 && normalizedNode.position.y === 0 && normalizedNode.position.z === 0)) {
            const position = getPositionForNode(normalizedNode, i, data.nodes.length)
            return { ...normalizedNode, position: { x: position[0], y: position[1], z: position[2] } }
          }
          return normalizedNode
        }),
        edges: data.edges.map((edge: any, idx: number) => {
          let src = edge.source ?? edge.from ?? edge.from_node ?? edge.sourceId ?? edge.source_id ?? edge.start
          let tgt = edge.target ?? edge.to ?? edge.to_node ?? edge.targetId ?? edge.target_id ?? edge.end
          if (src === 'undefined' || src === 'null' || src === '') src = undefined
          if (tgt === 'undefined' || tgt === 'null' || tgt === '') tgt = undefined
          if ((src == null || tgt == null) && edge.id && typeof edge.id === 'string') {
            const parts = edge.id.split('-')
            if (parts.length >= 2) {
              if (src == null) src = parts[0]
              if (tgt == null) tgt = parts.slice(1).join('-')
            }
          }
          if (idx === 0 && !(window as unknown as Record<string, boolean>).__edgeRecoveryDiag) {
            ;(window as unknown as Record<string, boolean>).__edgeRecoveryDiag = true
            logger.debug('[useGraphDataSubscription] edge[0] RECOVERY: src=', src, 'tgt=', tgt,
              'raw.source=', edge.source, 'raw.target=', edge.target, 'id=', edge.id)
          }
          return { ...edge, source: String(src), target: String(tgt) }
        }).filter((e: { source: string; target: string }) =>
          e.source !== 'undefined' && e.target !== 'undefined' && e.source !== 'null' && e.target !== 'null'
        ),
      }

      if (!(window as unknown as Record<string, boolean>).__edgePipelineV2) {
        ;(window as unknown as Record<string, boolean>).__edgePipelineV2 = true
        logger.debug('[useGraphDataSubscription] edge pipeline:',
          'inputEdges=', data.edges.length,
          'outputEdges=', dataWithPositions.edges.length,
          'nodes=', dataWithPositions.nodes.length,
          dataWithPositions.edges.length > 0
            ? { first: { src: dataWithPositions.edges[0].source, tgt: dataWithPositions.edges[0].target, id: dataWithPositions.edges[0].id } }
            : '(no edges survived filter)')
      }

      const allAtOrigin = dataWithPositions.nodes.every(node =>
        !node.position || (node.position.x === 0 && node.position.y === 0 && node.position.z === 0)
      )
      onNodesAtOrigin(allAtOrigin)
      onGraphData(dataWithPositions)

      // Seed initial edge points from node.position (before SAB data arrives)
      const posNodeMap = new Map(dataWithPositions.nodes.map(n => [String(n.id), n]))
      const newEdgePoints: number[] = []
      dataWithPositions.edges.forEach(edge => {
        const sourceNode = posNodeMap.get(String(edge.source))
        const targetNode = posNodeMap.get(String(edge.target))
        if (sourceNode?.position && targetNode?.position) {
          newEdgePoints.push(
            sourceNode.position.x, sourceNode.position.y, sourceNode.position.z,
            targetNode.position.x, targetNode.position.y, targetNode.position.z,
          )
        }
      })
      onEdgePoints(newEdgePoints)

      lastProcessedGraphRef.current = data
      lastShapeRef.current = shape
      return dataWithPositions
    }

    const unsubscribe = graphDataManager.onGraphDataChange(handleGraphUpdate)

    const fallbackTimer = window.setTimeout(() => {
      if (!lastProcessedGraphRef.current) {
        handleGraphUpdate({
          nodes: [
            { id: 'fallback1', label: 'Test Node 1', position: { x: -5, y: 0, z: 0 } },
            { id: 'fallback2', label: 'Test Node 2', position: { x:  5, y: 0, z: 0 } },
            { id: 'fallback3', label: 'Test Node 3', position: { x:  0, y: 5, z: 0 } },
          ],
          edges: [
            { id: 'fallback_edge1', source: 'fallback1', target: 'fallback2' },
            { id: 'fallback_edge2', source: 'fallback2', target: 'fallback3' },
          ],
        })
      }
    }, 5000)

    return () => {
      window.clearTimeout(fallbackTimer)
      unsubscribe()
    }
    // Deps intentionally narrowed; react-hooks/exhaustive-deps is not enforced in this config.
  }, [])
}
