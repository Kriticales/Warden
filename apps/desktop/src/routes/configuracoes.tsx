import { createFileRoute } from '@tanstack/react-router';

import { SettingsLayout } from '../features/settings/layout/SettingsLayout';

/** Configurações do app (T21): o menu lateral e a página aberta. */
export const Route = createFileRoute('/configuracoes')({
  component: SettingsLayout,
});
