import React, { createContext, useContext, useState } from 'react';

interface SwarmData {
  nodeCount: number;
  edgeCount: number;
  tokenCount: number;
  mcpConnected: boolean;
  dataSource: string;
}

interface SwarmDataContextType {
  swarmData: SwarmData | null;
  updateSwarmData: (data: SwarmData) => void;
}

const SwarmDataContext = createContext<SwarmDataContextType | undefined>(undefined);

export const SwarmDataProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [swarmData, setSwarmData] = useState<SwarmData | null>(null);

  const updateSwarmData = (data: SwarmData) => {
    setSwarmData(data);
  };

  return (
    <SwarmDataContext.Provider value={{ swarmData, updateSwarmData }}>
      {children}
    </SwarmDataContext.Provider>
  );
};

export const useSwarmData = () => {
  const context = useContext(SwarmDataContext);
  if (!context) {
    throw new Error('useSwarmData must be used within a SwarmDataProvider');
  }
  return context;
};