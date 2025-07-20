import React, { createContext, useContext, ReactNode } from 'react';
import { useOnboarding } from '../hooks/useOnboarding';
import { OnboardingOverlay } from './OnboardingOverlay';
import { OnboardingEventHandler } from './OnboardingEventHandler';
import { OnboardingFlow } from '../types';

interface OnboardingContextValue {
  startFlow: (flow: OnboardingFlow, forceRestart?: boolean) => boolean;
  resetOnboarding: () => void;
  completedFlows: string[];
}

const OnboardingContext = createContext<OnboardingContextValue | null>(null);

export function useOnboardingContext() {
  const context = useContext(OnboardingContext);
  if (!context) {
    throw new Error('useOnboardingContext must be used within OnboardingProvider');
  }
  return context;
}

interface OnboardingProviderProps {
  children: ReactNode;
}

export function OnboardingProvider({ children }: OnboardingProviderProps) {
  const {
    isActive,
    currentStep,
    currentStepIndex,
    currentFlow,
    completedFlows,
    hasNextStep,
    hasPrevStep,
    startFlow,
    nextStep,
    prevStep,
    skipFlow,
    resetOnboarding
  } = useOnboarding();

  const contextValue: OnboardingContextValue = {
    startFlow,
    resetOnboarding,
    completedFlows
  };

  return (
    <OnboardingContext.Provider value={contextValue}>
      {children}
      <OnboardingEventHandler />
      {isActive && currentStep && currentFlow && (
        <OnboardingOverlay
          step={currentStep}
          stepNumber={currentStepIndex + 1}
          totalSteps={currentFlow.steps.length}
          onNext={nextStep}
          onPrev={prevStep}
          onSkip={skipFlow}
          hasNext={hasNextStep}
          hasPrev={hasPrevStep}
        />
      )}
    </OnboardingContext.Provider>
  );
}