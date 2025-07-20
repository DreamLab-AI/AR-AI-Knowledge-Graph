import { useCallback, useEffect, useState } from 'react';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('useXRContextCheck');

/**
 * A hook that safely checks if the current component is being rendered
 * within a valid XR context, without causing errors when outside it.
 * 
 * @returns {boolean} isInXRContext - True if in valid XR context, false otherwise
 */
export const useXRContextCheck = (): boolean => {
  const [isInXRContext, setIsInXRContext] = useState<boolean>(false);

  useEffect(() => {
    // Check if we're in an XR context by checking for the '__r3f' property
    // which is added by react-three-fiber to elements in its render tree
    try {
      // If we can access THREE.WebXRManager or find XR elements in the DOM,
      // we're likely in an XR context
      const isInContext = typeof window !== 'undefined' && 
        ((document.querySelector('[data-xr-canvas="true"]') !== null) ||
         (document.querySelector('canvas.__r3f') !== null));
      
      setIsInXRContext(isInContext);
    } catch (error) {
      // If any error occurs during detection, assume we're not in XR context
      setIsInXRContext(false);
      logger.debug('XR context detection error, assuming outside context');
    }
  }, []);

  return isInXRContext;
};

export default useXRContextCheck;