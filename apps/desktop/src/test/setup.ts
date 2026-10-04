// Preparação comum dos testes de componente (Vitest + Testing Library + mockIPC).
import '../i18n';

import { clearMocks } from '@tauri-apps/api/mocks';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => {
  // Testes de ferramentas (`@vitest-environment node`) não têm `window`.
  if (typeof window !== 'undefined') {
    cleanup();
    clearMocks();
  }
});
