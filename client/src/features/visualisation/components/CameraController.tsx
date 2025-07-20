import { useEffect } from 'react';
import { useThree } from '@react-three/fiber';
import * as THREE from 'three'; // Use namespace import

interface CameraControllerProps {
  center: [number, number, number];
  size: number;
}

const CameraController: React.FC<CameraControllerProps> = ({ center, size }) => {
  const { camera } = useThree();

  useEffect(() => {
    // Ensure camera is PerspectiveCamera before accessing specific properties or methods
    if (camera instanceof THREE.PerspectiveCamera) {
        // Adjust position based on graph bounds
        camera.position.set(center[0], center[1] + 10, center[2] + size * 2);
        camera.lookAt(new THREE.Vector3(center[0], center[1], center[2])); // Use THREE.Vector3
        camera.updateProjectionMatrix();
    } else {
         console.warn("CameraController expects a PerspectiveCamera.");
         // Attempt basic adjustment anyway
         camera.position.set(center[0], center[1] + 10, center[2] + size * 2);
         camera.lookAt(new THREE.Vector3(center[0], center[1], center[2]));
    }
  }, [camera, center, size]);

  return null; // This component does not render anything itself
};

export default CameraController;
