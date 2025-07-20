import React from 'react';
import { Html } from '@react-three/drei';

interface DualVisualizationControlsProps {
    separationDistance: number;
    setSeparationDistance: (distance: number) => void;
}

export const DualVisualizationControls: React.FC<DualVisualizationControlsProps> = ({
    separationDistance,
    setSeparationDistance
}) => {
    return (
        <Html
            position={[0, 20, 0]}
            center
            style={{
                background: 'rgba(0, 0, 0, 0.8)',
                padding: '10px 20px',
                borderRadius: '8px',
                color: 'white',
                fontFamily: 'Inter, system-ui, sans-serif',
                pointerEvents: 'auto',
                userSelect: 'none'
            }}
        >
            <div style={{ textAlign: 'center' }}>
                <label style={{ display: 'block', marginBottom: '8px', fontSize: '14px' }}>
                    Graph Separation Distance: {separationDistance}
                </label>
                <input
                    type="range"
                    min="10"
                    max="50"
                    step="5"
                    value={separationDistance}
                    onChange={(e) => setSeparationDistance(Number(e.target.value))}
                    style={{
                        width: '200px',
                        cursor: 'pointer'
                    }}
                />
                <div style={{ marginTop: '8px', fontSize: '12px', opacity: 0.8 }}>
                    <span style={{ float: 'left' }}>Closer</span>
                    <span style={{ float: 'right' }}>Farther</span>
                </div>
            </div>
        </Html>
    );
};