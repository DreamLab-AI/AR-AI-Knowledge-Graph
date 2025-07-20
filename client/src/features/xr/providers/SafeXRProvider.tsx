import React, { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('SafeXRProvider');

interface XRContextProps {
  isXRCapable: boolean;
  isXRSupported: boolean;
}

const XRContext = createContext<XRContextProps>({
  isXRCapable: false,
  isXRSupported: false,
});

export const useXR = () => useContext(XRContext);

interface SafeXRProviderProps {
  children: ReactNode;
}

const SafeXRProvider: React.FC<SafeXRProviderProps> = ({ children }) => {
  const [isXRCapable, setIsXRCapable] = useState(false);
  const [isXRSupported, setIsXRSupported] = useState(false);
  const { settings } = useSettingsStore();

  useEffect(() => {
    const checkXRSupport = async () => {
      try {
        if ('xr' in navigator) {
          const supported = await (navigator.xr as any).isSessionSupported('immersive-vr');
          setIsXRSupported(supported);
          setIsXRCapable(true);
          logger.info('XR is capable and immersive VR is supported.');
        } else {
          setIsXRCapable(false);
          setIsXRSupported(false);
          logger.warn('XR is not available in this browser.');
        }
      } catch (error) {
        setIsXRCapable(false);
        setIsXRSupported(false);
        logger.error('Error checking XR support:', error);
      }
    };

    checkXRSupport();
  }, []);

  useEffect(() => {
      const debugEnabled = settings?.system?.debug?.enabled === true
    if (debugEnabled) {
      logger.info(`XR capability changed: capable=${isXRCapable}, supported=${isXRSupported}`);
    }
  }, [isXRCapable, isXRSupported, settings?.system?.debug?.enabled]);

  return (
    <XRContext.Provider value={{ isXRCapable, isXRSupported }}>
      {children}
    </XRContext.Provider>
  );
};

export default SafeXRProvider;