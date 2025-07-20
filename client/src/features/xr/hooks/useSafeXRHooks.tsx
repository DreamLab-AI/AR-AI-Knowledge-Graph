import React from 'react';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('safeXRHooks');

// Default empty values for XR-related state when outside XR context
const emptyXRState = {
  isPresenting: false,
  controllers: [],
  player: null,
  session: null,
  hoverState: {},
};

/**
 * A safe version of useXR that won't throw errors when used outside XR context
 * This allows XR components to be rendered anywhere without errors
 */
export function useSafeXR() {
  try {
    // Try to dynamically import @react-three/xr hooks
    // This is needed because direct import at the top level would still cause errors
    const { useXR } = require('@react-three/xr');
    
    // If we get here, we can safely use the hook
    return useXR();
  } catch (error) {
    // If accessing the XR hook fails, return default values
    // to prevent component errors
    logger.debug('Using safe XR fallback - component outside XR context');
    return emptyXRState;
  }
}

/**
 * A safe wrapper for creating XR-dependent components
 * that won't throw errors when used outside XR context
 */
export function withSafeXR<P extends object>(
  Component: React.ComponentType<P>,
  componentName: string = 'Component'
): React.FC<P> {
  const WrappedComponent: React.FC<P> = (props) => {
    try {
      // Try to render component normally
      return <Component {...props} />;
    } catch (error) {
      // If an XR-related error occurs, don't render the component
      logger.debug(`${componentName} not rendered - XR context error`);
      return null;
    }
  };
  
  // Set a display name for better debugging
  WrappedComponent.displayName = `withSafeXR(${componentName})`;
  
  return WrappedComponent;
}