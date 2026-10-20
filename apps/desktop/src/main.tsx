import './i18n';
import './styles/app.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './app/App';
import { installAppBehavior } from './app/app-behavior';

// App, não página: sem menu de contexto fora dos campos e sem voltar/avançar do mouse (ADR-0063).
installAppBehavior();

const container = document.getElementById('root');
if (container) {
  createRoot(container).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
