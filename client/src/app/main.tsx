import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { initializeDebugSystem } from '../utils/debugConfig';
import '../styles/index.css'; // Use relative path

// Initialize debug system before app starts
initializeDebugSystem();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
