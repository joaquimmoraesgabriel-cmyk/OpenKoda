import React from 'react';
import { createRoot } from 'react-dom/client';
import '../src/styles.css';
import App from '../src/App';
import './demo.css';
createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
