import React, { createContext, useContext, useState } from 'react';

interface BotsData {
  nodeCount: number;
  edgeCount: number;
  tokenCount: number;
  mcpConnected: boolean;
  dataSource: string;
}

interface BotsDataContextType {
  botsData: BotsData | null;
  updateBotsData: (data: BotsData) => void;
}

const BotsDataContext = createContext<BotsDataContextType | undefined>(undefined);

export const BotsDataProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [botsData, setBotsData] = useState<BotsData | null>({
    nodeCount: 0,
    edgeCount: 0,
    tokenCount: 0,
    mcpConnected: false,
    dataSource: 'mock'
  });

  const updateBotsData = (data: BotsData) => {
    setBotsData(data);
  };

  return (
    <BotsDataContext.Provider value={{ botsData, updateBotsData }}>
      {children}
    </BotsDataContext.Provider>
  );
};

export const useBotsData = () => {
  const context = useContext(BotsDataContext);
  if (!context) {
    throw new Error('useBotsData must be used within a BotsDataProvider');
  }
  return context;
};