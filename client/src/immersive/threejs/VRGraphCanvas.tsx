import React, { Suspense, useState, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import { createXRStore, XR } from '@react-three/xr';
import GraphManager from '../../features/graph/components/GraphManager';

interface VRGraphCanvasProps {
  onDragStateChange?: (isDragging: boolean) => void;
}

// Create XR store outside component to persist across renders
// Disable XR emulation in production to avoid bundling @iwer/sem room scene
// data (~4.6MB of MetaQuest scene captures used only for localhost dev).
const xrStore = createXRStore({
  hand: true,
  controller: true,
  emulate: import.meta.env.DEV ? 'metaQuest3' : false,
});

export function VRGraphCanvas({
  onDragStateChange,
}: VRGraphCanvasProps) {
  const [isVRSupported, setIsVRSupported] = useState<boolean | null>(null);

  // Check for ?vr=true URL parameter to force VR mode on Quest 3
  const forceVR = useMemo(() => {
    const urlParams = new URLSearchParams(window.location.search);
    return urlParams.get('vr') === 'true';
  }, []);

  // Check VR support on mount
  React.useEffect(() => {
    if (navigator.xr) {
      navigator.xr.isSessionSupported('immersive-vr').then(setIsVRSupported);
    } else {
      setIsVRSupported(false);
    }
  }, []);

  return (
    <>
      {(isVRSupported || forceVR) && (
        <button
          onClick={() => xrStore.enterVR()}
          style={{
            position: 'absolute',
            bottom: '20px',
            left: '50%',
            transform: 'translateX(-50%)',
            padding: '12px 24px',
            fontSize: '16px',
            backgroundColor: '#4CAF50',
            color: 'white',
            border: 'none',
            borderRadius: '8px',
            cursor: 'pointer',
            zIndex: 1000,
          }}
        >
          Enter VR
        </button>
      )}
      <Canvas
        gl={{ antialias: true, alpha: false }}
        camera={{ position: [0, 1.6, 3], fov: 70 }}
      >
        <XR store={xrStore}>
          <Suspense fallback={null}>
            <ambientLight intensity={0.5} />
            <pointLight position={[10, 10, 10]} />
            {/* GraphManager mounts TransientBeamsLayer — the canonical agent-action
                renderer on both desktop and VR since 2026-07-15. */}
            <GraphManager onDragStateChange={onDragStateChange} />
          </Suspense>
        </XR>
      </Canvas>
    </>
  );
}

export default VRGraphCanvas;
