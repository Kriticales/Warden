// Preparação comum dos testes de componente (Vitest + Testing Library + mockIPC + vitest-axe).
import '../i18n';

import { clearMocks } from '@tauri-apps/api/mocks';
import { cleanup, configure } from '@testing-library/react';
import { afterEach, expect } from 'vitest';
import * as axeMatchers from 'vitest-axe/matchers';

import { useTasksUi } from '../app/tasks/tasks-store';
import { useToastStore } from '../components/ui/toast';

expect.extend(axeMatchers);

// O primeiro `renderApp` de cada arquivo carrega o app inteiro (rotas, telas e traduções) e,
// sob carga, passa de 1 s (o padrão do `findBy`/`waitFor`). O teto é só um limite: as esperas
// seguem o estado da tela e terminam assim que ele aparece.
configure({ asyncUtilTimeout: 5000 });

// O jsdom não tem a captura de ponteiro que o Radix usa (toast que se arrasta para fechar).
if (typeof window !== 'undefined' && !('hasPointerCapture' in Element.prototype)) {
  Object.assign(Element.prototype, {
    hasPointerCapture: () => false,
    setPointerCapture: () => undefined,
    releasePointerCapture: () => undefined,
  });
}

// O jsdom não mede texto: o CodeMirror (editor de configs) pergunta pelos retângulos do Range.
if (typeof Range !== 'undefined' && !('getClientRects' in Range.prototype)) {
  const rect = { x: 0, y: 0, width: 0, height: 0, top: 0, right: 0, bottom: 0, left: 0 };
  Object.assign(Range.prototype, {
    getClientRects: () => [],
    getBoundingClientRect: () => ({ ...rect, toJSON: () => rect }),
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
