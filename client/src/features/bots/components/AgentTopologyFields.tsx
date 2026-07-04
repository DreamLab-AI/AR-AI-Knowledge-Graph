import React from 'react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../design-system/components/Select';
import { Slider } from '../../design-system/components/Slider';
import { Switch } from '../../design-system/components/Switch';

export type Topology = 'mesh' | 'hierarchical' | 'ring' | 'star';

const TOPOLOGY_OPTIONS: { value: Topology; label: string }[] = [
  { value: 'mesh', label: 'Mesh — fully connected, best for collaboration' },
  { value: 'hierarchical', label: 'Hierarchical — structured with clear command chain' },
  { value: 'ring', label: 'Ring — sequential processing pipeline' },
  { value: 'star', label: 'Star — central coordinator with workers' },
];

export interface AgentTopologyFieldsProps {
  topology: Topology;
  onTopologyChange: (value: Topology) => void;
  maxAgents: number;
  onMaxAgentsChange: (value: number) => void;
  enableNeural: boolean;
  onEnableNeuralChange: (value: boolean) => void;
}

/** Topology picker, max-agents slider, and the neural-enhancement toggle. */
export const AgentTopologyFields: React.FC<AgentTopologyFieldsProps> = ({
  topology,
  onTopologyChange,
  maxAgents,
  onMaxAgentsChange,
  enableNeural,
  onEnableNeuralChange,
}) => {
  return (
    <>
      <div className="mb-4">
        <span className="cc-field-label mb-1.5 block">Topology</span>
        <Select value={topology} onValueChange={(v) => onTopologyChange(v as Topology)}>
          <SelectTrigger className="h-9 truncate text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TOPOLOGY_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value} className="text-xs">
                {opt.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="mb-4">
        <div className="cc-row-header mb-1.5">
          <span className="cc-field-label">Maximum Agents</span>
          <span className="cc-value-readout">{maxAgents}</span>
        </div>
        <Slider
          value={[maxAgents]}
          onValueChange={([v]) => onMaxAgentsChange(v)}
          min={3}
          max={20}
          step={1}
        />
      </div>

      <div className="mb-4">
        <label className="flex cursor-pointer items-center justify-between">
          <span className="cc-field-label">Enable Neural Enhancements</span>
          <Switch checked={enableNeural} onCheckedChange={onEnableNeuralChange} />
        </label>
        <p className="cc-helper-text mt-1">
          Activates WASM-accelerated neural networks for collective intelligence
        </p>
      </div>
    </>
  );
};

export default AgentTopologyFields;
