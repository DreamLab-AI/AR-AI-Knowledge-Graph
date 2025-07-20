import { lazy } from 'react';

// Lazy load heavy setting sections
export const LazyAdvancedSettings = lazy(() => 
  import('./panels/AdvancedSettingsPanel').then(m => ({ default: m.AdvancedSettingsPanel }))
);

export const LazyXRSettings = lazy(() => 
  import('./panels/XRPanel').then(m => ({ default: m.XRPanel }))
);

export const LazyVisualizationSettings = lazy(() => 
  import('./panels/VisualisationPanel').then(m => ({ default: m.VisualisationPanel }))
);

export const LazyAISettings = lazy(() => 
  import('./panels/AIPanel').then(m => ({ default: m.AIPanel }))
);

export const LazySystemSettings = lazy(() => 
  import('./panels/SystemPanel').then(m => ({ default: m.SystemPanel }))
);

// Wrapper component for lazy loading with fallback
import React from 'react';
import { LoadingSpinner } from '@/ui/LoadingSpinner';

interface LazySettingSectionProps {
  component: React.LazyExoticComponent<React.ComponentType<any>>;
  props?: any;
}

export const LazySettingSection: React.FC<LazySettingSectionProps> = ({ component: Component, props = {} }) => {
  return (
    <React.Suspense 
      fallback={
        <div className="flex items-center justify-center h-32">
          <LoadingSpinner />
        </div>
      }
    >
      <Component {...props} />
    </React.Suspense>
  );
};