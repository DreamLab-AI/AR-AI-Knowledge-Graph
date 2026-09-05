import React, { createContext, useContext, useState, useEffect, useMemo, ReactNode } from 'react';
import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('ApplicationModeContext');

type ApplicationMode = 'desktop' | 'mobile';

interface LayoutSettings {
  showPanels: boolean;
  showViewport: boolean;
  showControls: boolean;
}

interface ApplicationModeContextValue {
  mode: ApplicationMode;
  previousMode: ApplicationMode | null;
  isMobileView: boolean;
  setMode: (mode: ApplicationMode) => void;
  layoutSettings: LayoutSettings;
}

const defaultContext: ApplicationModeContextValue = {
  mode: 'desktop',
  previousMode: null,
  isMobileView: false,
  setMode: () => { },
  layoutSettings: {
    showPanels: true,
    showViewport: true,
    showControls: true
  }
};

// Create the context
const ApplicationModeContext = createContext<ApplicationModeContextValue>(defaultContext);

interface ApplicationModeProviderProps {
  children: ReactNode;
}

export const ApplicationModeProvider: React.FC<ApplicationModeProviderProps> = ({ children }) => {
  const [mode, setMode] = useState<ApplicationMode>('desktop');
  const [previousMode, setPreviousMode] = useState<ApplicationMode | null>(null);
  const [isMobileView, setIsMobileView] = useState(false);

  useEffect(() => {
    const handleResize = () => {
      const isMobile = window.innerWidth < 768;
      setIsMobileView(isMobile);

      if (isMobile) {
        setMode('mobile');
      }
      else if (!isMobile && mode === 'mobile') {
        setMode('desktop');
      }
    };

    handleResize();

    window.addEventListener('resize', handleResize);

    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, [mode]);

  const handleModeChange = (newMode: ApplicationMode) => {
    logger.info(`Changing mode: ${mode} -> ${newMode}`);
    setPreviousMode(mode);
    setMode(newMode);
  };
    
  const getLayoutSettings = (): LayoutSettings => {
    switch (mode) {
      case 'desktop':
        return {
          showPanels: true,
          showViewport: true,
          showControls: true
        };
      case 'mobile':
        return {
          showPanels: true,
          showViewport: true,
          showControls: true
        };
      default:
        return {
          showPanels: true,
          showViewport: true,
          showControls: true
        };
    }
  };

  const layoutSettings = useMemo(() => getLayoutSettings(), [mode]);

  const contextValue: ApplicationModeContextValue = {
    mode,
    previousMode,
    isMobileView,
    setMode: handleModeChange,
    layoutSettings
  };

  return (
    <ApplicationModeContext.Provider value={contextValue}>
      {children}
    </ApplicationModeContext.Provider>
  );
};

export const useApplicationMode = () => {
    const context = useContext(ApplicationModeContext);
    if (!context) {
        throw new Error('useApplicationMode must be used within an ApplicationModeProvider');
    }
    return context;
};
