// Preparação comum dos testes de componente (Vitest + Testing Library + mockIPC + vitest-axe).
import '../i18n';

import { clearMocks } from '@tauri-apps/api/mocks';
import { cleanup } from '@testing-library/react';
import { afterEach, expect } from 'vitest';
import * as axeMatchers from 'vitest-axe/matchers';

import { useTasksUi } from '../app/tasks/tasks-store';
import { useToastStore } from '../components/ui/toast';

expect.extend(axeMatchers);

// O jsdom não tem a captura de ponteiro que o Radix usa (toast que se arrasta para fechar).
if (typeof window !== 'undefined' && !('hasPointerCapture' in Element.prototype)) {
  Object.assign(Element.prototype, {
    hasPointerCapture: () => false,
    setPointerCapture: () => undefined,
    releasePointerCapture: () => undefined,
  });
}

afterEach(() => {
  // Testes de ferramentas (`@vitest-environment node`) não têm `window`.
  if (typeof window !== 'undefined') {
    cleanup();
    clearMocks();
    useToastStore.getState().clear();
    useTasksUi.setState({ open: false, seenUntilMs: 0, expandedId: null });
  }
});
