import { useState, useCallback, useEffect } from 'react';
import { OnboardingState, OnboardingFlow, OnboardingStep } from '../types';

const COMPLETED_FLOWS_KEY = 'onboarding.completedFlows';

export function useOnboarding() {
  const [state, setState] = useState<OnboardingState>({
    isActive: false,
    currentFlow: null,
    currentStepIndex: 0,
    completedFlows: []
  });

  // Load completed flows from localStorage
  useEffect(() => {
    try {
      const stored = localStorage.getItem(COMPLETED_FLOWS_KEY);
      if (stored) {
        setState(prev => ({
          ...prev,
          completedFlows: JSON.parse(stored)
        }));
      }
    } catch (error) {
      console.error('Failed to load onboarding state:', error);
    }
  }, []);

  // Start onboarding flow
  const startFlow = useCallback((flow: OnboardingFlow, forceRestart = false) => {
    if (!forceRestart && state.completedFlows.includes(flow.id)) {
      return false;
    }

    setState(prev => ({
      ...prev,
      isActive: true,
      currentFlow: flow,
      currentStepIndex: 0
    }));

    return true;
  }, [state.completedFlows]);

  // Navigate to next step
  const nextStep = useCallback(async () => {
    if (!state.currentFlow) return;

    const currentStep = state.currentFlow.steps[state.currentStepIndex];
    
    // Execute step action if provided
    if (currentStep.action) {
      try {
        await currentStep.action();
      } catch (error) {
        console.error('Failed to execute step action:', error);
      }
    }

    if (state.currentStepIndex < state.currentFlow.steps.length - 1) {
      setState(prev => ({
        ...prev,
        currentStepIndex: prev.currentStepIndex + 1
      }));
    } else {
      // Flow completed
      completeFlow();
    }
  }, [state.currentFlow, state.currentStepIndex]);

  // Navigate to previous step
  const prevStep = useCallback(() => {
    if (state.currentStepIndex > 0) {
      setState(prev => ({
        ...prev,
        currentStepIndex: prev.currentStepIndex - 1
      }));
    }
  }, [state.currentStepIndex]);

  // Skip current flow
  const skipFlow = useCallback(() => {
    completeFlow();
  }, []);

  // Complete current flow
  const completeFlow = useCallback(() => {
    if (!state.currentFlow) return;

    const flowId = state.currentFlow.id;
    const newCompletedFlows = [...state.completedFlows, flowId];

    // Save to localStorage
    try {
      localStorage.setItem(COMPLETED_FLOWS_KEY, JSON.stringify(newCompletedFlows));
    } catch (error) {
      console.error('Failed to save onboarding state:', error);
    }

    setState(prev => ({
      ...prev,
      isActive: false,
      currentFlow: null,
      currentStepIndex: 0,
      completedFlows: newCompletedFlows
    }));

    // Emit completion event
    window.dispatchEvent(new CustomEvent('onboarding-completed', { 
      detail: { flowId } 
    }));
  }, [state.currentFlow, state.completedFlows]);

  // Reset onboarding state
  const resetOnboarding = useCallback(() => {
    localStorage.removeItem(COMPLETED_FLOWS_KEY);
    setState({
      isActive: false,
      currentFlow: null,
      currentStepIndex: 0,
      completedFlows: []
    });
  }, []);

  // Get current step
  const currentStep = state.currentFlow 
    ? state.currentFlow.steps[state.currentStepIndex]
    : null;

  const hasNextStep = state.currentFlow 
    ? state.currentStepIndex < state.currentFlow.steps.length - 1
    : false;

  const hasPrevStep = state.currentStepIndex > 0;

  return {
    ...state,
    currentStep,
    hasNextStep,
    hasPrevStep,
    startFlow,
    nextStep,
    prevStep,
    skipFlow,
    completeFlow,
    resetOnboarding
  };
}