import React from 'react';
import ReactDOM from 'react-dom/client';
import './styles.css';
import App from './App';
import { Startup } from './components/DesktopEntry';

// This contains only the last chosen color theme, never account or project data.
try {
  const theme=localStorage.getItem('openkoda.boot-theme.v1');
  if(theme&&['light','dark','system'].includes(theme))document.documentElement.dataset.theme=theme;
} catch { /* Startup remains available when device preferences cannot be read. */ }

class DesktopErrorBoundary extends React.Component<{children:React.ReactNode},{failed:boolean}> {
  state={failed:false};
  static getDerivedStateFromError(){return {failed:true};}
  render(){return this.state.failed?<Startup error="A interface encontrou um problema ao abrir. Reinicie o aplicativo para tentar novamente." onRetry={()=>window.location.reload()}/>:this.props.children;}
}
ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><DesktopErrorBoundary><App /></DesktopErrorBoundary></React.StrictMode>);
