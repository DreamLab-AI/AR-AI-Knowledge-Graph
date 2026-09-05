import { useEffect } from 'react';
import { useThree, useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { useSettingsStore } from '@/store/settingsStore';
import { useHeadTracking } from '@/hooks/useHeadTracking';
import { toast } from '@/features/design-system/components/Toast';

// ADR-04 D10: pre-allocated scratch for the per-frame projection nudge below.
// makeTranslation overwrites every element before each use, so there is no
// carry-over between frames and no per-frame allocation.
const NUDGE_MATRIX = new THREE.Matrix4();

export function HeadTrackedParallaxController() {
  const { camera, size } = useThree();
  const { isEnabled, setIsEnabled, isTracking, headPosition, error } = useHeadTracking();

  const trackingEnabled = useSettingsStore(state => state.settings?.visualisation?.interaction?.headTrackedParallax?.enabled);
  const sensitivity = useSettingsStore(state => state.settings?.visualisation?.interaction?.headTrackedParallax?.sensitivity ?? 1.0);
  const cameraMode = useSettingsStore(state => state.settings?.visualisation?.interaction?.headTrackedParallax?.cameraMode ?? 'asymmetricFrustum');

  useEffect(() => {
    setIsEnabled(!!trackingEnabled);
  }, [trackingEnabled, setIsEnabled]);

  useEffect(() => {
    if (error) {
      toast({
        title: 'Head Tracking Error',
        description: error,
        variant: 'destructive',
      });
    }
  }, [error]);

  useFrame(() => {
    if (isTracking && headPosition && camera instanceof THREE.PerspectiveCamera) {
      if (cameraMode === 'asymmetricFrustum') {
        
        const virtualScreenScale = 1.0 + sensitivity * 0.5;
        const fullWidth = size.width * virtualScreenScale;
        const fullHeight = size.height * virtualScreenScale;

        const x_offset = -headPosition.x * (fullWidth - size.width) / 2;
        const y_offset = headPosition.y * (fullHeight - size.height) / 2;

        camera.setViewOffset(
          fullWidth,
          fullHeight,
          x_offset,
          y_offset,
          size.width,
          size.height
        );
        camera.updateProjectionMatrix();
      } else {
        
        const offsetX = headPosition.x * sensitivity * -0.5;
        const offsetY = headPosition.y * sensitivity * 0.5;

        // The intermediate Vector3 here was dead weight — only .x/.y were read
        // straight back out — and both allocations ran on EVERY tracked frame.
        camera.projectionMatrix.multiply(NUDGE_MATRIX.makeTranslation(offsetX, offsetY, 0));
      }
    } else {
      
      if (camera instanceof THREE.PerspectiveCamera && camera.view) {
        camera.clearViewOffset();
        camera.updateProjectionMatrix();
      }
    }
  });

  return null;
}
