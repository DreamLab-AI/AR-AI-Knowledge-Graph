import React, { useState, useCallback } from 'react'
import { useXRCore } from '../providers/XRCoreProvider'
import HandInteractionSystem, { GestureRecognitionResult } from '../systems/HandInteractionSystem'
import { useSettingsStore } from '../../../store/settingsStore'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('XRController')

/**
 * XRController component manages Quest 3 AR functionality and hand tracking.
 * Uses the enhanced XRCoreProvider for robust session management.
 */
const XRController: React.FC = () => {
  const { isSessionActive, sessionType, controllers } = useXRCore()
  const settings = useSettingsStore(state => state.settings)
  const [handsVisible, setHandsVisible] = useState(false)
  const [handTrackingEnabled, setHandTrackingEnabled] = useState(settings?.xr?.enableHandTracking !== false)
  
  // Log session state changes (Quest 3 AR focused)
  React.useEffect(() => {
    if (settings?.system?.debug?.enabled) {
      if (isSessionActive && sessionType === 'immersive-ar') {
        logger.info('Quest 3 AR session is now active')
      } else if (isSessionActive) {
        logger.info('XR session active but not Quest 3 AR mode')
      } else {
        logger.info('XR session is not active')
      }
    }
  }, [isSessionActive, sessionType, settings?.system?.debug?.enabled])

  // Log controller information
  React.useEffect(() => {
    if (isSessionActive && controllers && controllers.length > 0 && settings?.system?.debug?.enabled) {
      logger.info(`Quest 3 controllers active: ${controllers.length}`)
      controllers.forEach((controller, index) => {
        logger.info(`Controller ${index}: Three.js XRTargetRaySpace object`)
      })
    }
  }, [controllers, isSessionActive, settings?.system?.debug?.enabled])

  // Handle gesture recognition (Quest 3 optimized)
  const handleGestureRecognized = useCallback((gesture: GestureRecognitionResult) => {
    if (settings?.system?.debug?.enabled) {
      logger.info(`Quest 3 gesture recognized: ${gesture.gesture} (${gesture.confidence.toFixed(2)}) with ${gesture.hand} hand`)
    }
  }, [settings?.system?.debug?.enabled])

  // Handle hand visibility changes
  const handleHandsVisible = useCallback((visible: boolean) => {
    setHandsVisible(visible)
    
    if (settings?.system?.debug?.enabled) {
      logger.info(`Quest 3 hands visible: ${visible}`)
    }
  }, [settings?.system?.debug?.enabled])
  
  // Only render if XR enabled and preferably in Quest 3 AR mode
  if (settings?.xr?.enabled === false) {
    return null
  }
  
  // Prioritize Quest 3 AR mode
  const isQuest3AR = isSessionActive && sessionType === 'immersive-ar'
  
  return (
    <group name="quest3-ar-controller-root">
      <HandInteractionSystem
        enabled={handTrackingEnabled && (isQuest3AR || isSessionActive)}
        onGestureRecognized={handleGestureRecognized}
        onHandsVisible={handleHandsVisible}
      />
    </group>
  )
}

export default XRController