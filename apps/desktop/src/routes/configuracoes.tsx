import { createFileRoute } from '@tanstack/react-router';

import { SettingsPage } from '../features/settings/components/SettingsPage';

/** Configurações do app (T21). */
export const Route = createFileRoute('/configuracoes')({
  component: SettingsPage,
});
