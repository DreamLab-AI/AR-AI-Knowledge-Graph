import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { createLogger } from '../utils/logger'
import { useRef, useEffect } from 'react'

const logger = createLogger('MultiUserStore')

export interface UserData {
  id: string
  name?: string
  position: [number, number, number]
  rotation: [number, number, number]
  isSelecting: boolean
  selectedNodeId?: string
  lastUpdate: number
  color?: string
}

interface MultiUserState {
  localUserId: string
  users: Record<string, UserData>
  connectionStatus: 'disconnected' | 'connecting' | 'connected'

  // Actions
  setLocalUserId: (userId: string) => void
  updateUser: (userId: string, data: Partial<UserData>) => void
  removeUser: (userId: string) => void
  updateLocalPosition: (position: [number, number, number], rotation: [number, number, number]) => void
  updateLocalSelection: (isSelecting: boolean, selectedNodeId?: string) => void
  setConnectionStatus: (status: 'disconnected' | 'connecting' | 'connected') => void
  clearStaleUsers: (staleThreshold?: number) => void
}

export const useMultiUserStore = create<MultiUserState>()(
  subscribeWithSelector((set, get) => ({
    localUserId: '',
    users: {},
    connectionStatus: 'disconnected',

    setLocalUserId: (userId) => {
      set({ localUserId: userId })
      logger.info('Local user ID set:', userId)
    },

    updateUser: (userId, data) => {
      set((state) => ({
        users: {
          ...state.users,
          [userId]: {
            ...state.users[userId],
            ...data,
            id: userId,
            lastUpdate: Date.now()
          }
        }
      }))
    },

    removeUser: (userId) => {
      set((state) => {
        const { [userId]: removed, ...remaining } = state.users
        logger.info('User removed:', userId)
        return { users: remaining }
      })
    },

    updateLocalPosition: (position, rotation) => {
      const { localUserId, updateUser } = get()
      if (localUserId) {
        updateUser(localUserId, { position, rotation })
      }
    },

    updateLocalSelection: (isSelecting, selectedNodeId) => {
      const { localUserId, updateUser } = get()
      if (localUserId) {
        updateUser(localUserId, { isSelecting, selectedNodeId })
      }
    },

    setConnectionStatus: (status) => {
      set({ connectionStatus: status })
      logger.info('Connection status:', status)
    },

    clearStaleUsers: (staleThreshold = 30000) => {
      const now = Date.now()
      set((state) => {
        const activeUsers = Object.entries(state.users).reduce((acc, [userId, userData]) => {
          if (now - userData.lastUpdate < staleThreshold || userId === state.localUserId) {
            acc[userId] = userData
          } else {
            logger.info('Removing stale user:', userId)
          }
          return acc
        }, {} as Record<string, UserData>)

        return { users: activeUsers }
      })
    }
  }))
)

// WebSocket connection manager for multi-user synchronization
export class MultiUserConnection {
  private ws: WebSocket | null = null
  private reconnectInterval: NodeJS.Timeout | null = null
  private heartbeatInterval: NodeJS.Timeout | null = null

  constructor(private url: string) {}

  connect() {
    const store = useMultiUserStore.getState()
    store.setConnectionStatus('connecting')

    try {
      this.ws = new WebSocket(this.url)

      this.ws.onopen = () => {
        logger.info('WebSocket connected')
        store.setConnectionStatus('connected')
        this.startHeartbeat()

        // Send initial user data
        const { localUserId } = store
        if (localUserId) {
          this.send({
            type: 'join',
            userId: localUserId,
            timestamp: Date.now()
          })
        }
      }

      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data)
          this.handleMessage(message)
        } catch (error) {
          logger.error('Failed to parse message:', error)
        }
      }

      this.ws.onclose = () => {
        logger.info('WebSocket disconnected')
        store.setConnectionStatus('disconnected')
        this.stopHeartbeat()
        this.scheduleReconnect()
      }

      this.ws.onerror = (error) => {
        logger.error('WebSocket error:', error)
      }
    } catch (error) {
      logger.error('Failed to connect:', error)
      store.setConnectionStatus('disconnected')
      this.scheduleReconnect()
    }
  }

  private handleMessage(message: any) {
    const store = useMultiUserStore.getState()

    switch (message.type) {
      case 'userUpdate':
        if (message.userId !== store.localUserId) {
          store.updateUser(message.userId, message.data)
        }
        break

      case 'userLeft':
        store.removeUser(message.userId)
        break

      case 'sync':
        // Full state sync from server
        Object.entries(message.users).forEach(([userId, userData]) => {
          if (userId !== store.localUserId) {
            store.updateUser(userId, userData as Partial<UserData>)
          }
        })
        break

      case 'pong':
        // Heartbeat response
        break

      default:
        logger.warn('Unknown message type:', message.type)
    }
  }

  send(data: any) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data))
    }
  }

  disconnect() {
    this.stopHeartbeat()
    if (this.reconnectInterval) {
      clearInterval(this.reconnectInterval)
      this.reconnectInterval = null
    }
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
  }

  private startHeartbeat() {
    this.heartbeatInterval = setInterval(() => {
      this.send({ type: 'ping', timestamp: Date.now() })
    }, 5000)
  }

  private stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval)
      this.heartbeatInterval = null
    }
  }

  private scheduleReconnect() {
    if (this.reconnectInterval) return

    this.reconnectInterval = setInterval(() => {
      logger.info('Attempting to reconnect...')
      this.connect()
    }, 5000)
  }
}

// Hook for tracking user position in XR
export const useXRUserTracking = () => {
  const updateLocalPosition = useMultiUserStore(state => state.updateLocalPosition)
  const updateLocalSelection = useMultiUserStore(state => state.updateLocalSelection)
  const connection = useRef<MultiUserConnection | null>(null)

  // Subscribe to position updates
  useEffect(() => {
    const unsubscribe = useMultiUserStore.subscribe(
      state => state.users[state.localUserId],
      (userData) => {
        if (userData && connection.current) {
          connection.current.send({
            type: 'userUpdate',
            userId: userData.id,
            data: {
              position: userData.position,
              rotation: userData.rotation,
              isSelecting: userData.isSelecting,
              selectedNodeId: userData.selectedNodeId
            }
          })
        }
      }
    )

    return unsubscribe
  }, [])

  return {
    updatePosition: updateLocalPosition,
    updateSelection: updateLocalSelection
  }
}