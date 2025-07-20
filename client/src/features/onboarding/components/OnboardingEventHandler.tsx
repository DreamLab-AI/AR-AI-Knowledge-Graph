import { useEffect } from 'react';
import { useOnboardingContext } from './OnboardingProvider';
import { welcomeFlow, settingsFlow, advancedFlow } from '../flows/defaultFlows';

const flowMap: Record<string, any> = {
  'welcome': welcomeFlow,
  'settings-tour': settingsFlow,
  'advanced-features': advancedFlow
};

export function OnboardingEventHandler() {
  const { startFlow, resetOnboarding } = useOnboardingContext();

  useEffect(() => {
    const handleStartOnboarding = (event: CustomEvent) => {
      const flowId = event.detail?.flowId;
      const flow = flowMap[flowId];
      if (flow) {
        startFlow(flow, event.detail?.forceRestart);
      }
    };

    const handleStartTour = () => {
      startFlow(welcomeFlow, true);
    };

    const handleResetOnboarding = () => {
      resetOnboarding();
    };

    window.addEventListener('start-onboarding', handleStartOnboarding as EventListener);
    window.addEventListener('start-tour', handleStartTour);
    window.addEventListener('reset-onboarding', handleResetOnboarding);

    return () => {
      window.removeEventListener('start-onboarding', handleStartOnboarding as EventListener);
      window.removeEventListener('start-tour', handleStartTour);
      window.removeEventListener('reset-onboarding', handleResetOnboarding);
    };
  }, [startFlow, resetOnboarding]);

  return null;
}