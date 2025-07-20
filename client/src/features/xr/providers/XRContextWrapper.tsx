import React from 'react';
import { useXR } from '@react-three/xr';
import { createLogger } from '@/utils/logger';

const logger = createLogger('XRContextWrapper');

/**
 * A higher-order component (HOC) that safely wraps components that use XR features.
 * This prevents "XR features can only be used inside the <XR> component" errors by
 * checking if we're in a valid XR context before rendering the wrapped component.
 */
export const withXRContext = <P extends object>(
  Component: React.ComponentType<P>,
  componentName: string = 'Component'
): React.FC<P> => {
  const WrappedComponent: React.FC<P> = (props) => {
    try {
      // This will throw an error if we're outside an XR context
      const xr = useXR();
      
      // If we get here, the XR context is valid
      return <Component {...props} />;
    } catch (error) {
      // If we're here, we're outside an XR context
      logger.debug(`Not rendering ${componentName} - outside XR context`);
      return null;
    }
  };
  
  // Set a display name for better debugging
  WrappedComponent.displayName = `withXRContext(${componentName})`;
  
  return WrappedComponent;
};

export default withXRContext;