import { createFileRoute, notFound } from '@tanstack/react-router';

import type { SecretKind } from '../lib/ipc/bindings';
import { SettingsPageView } from '../features/settings/pages/SettingsPageView';
import { isSettingsPage } from '../features/settings/pages';

const SECRET_KINDS: readonly SecretKind[] = [
  'curseforge',
  'github',
  'anthropic',
  'gemini',
  'openai',
];

export interface SettingsSearch {
  /** Chaves e contas: a chave escolhida. */
  chave?: SecretKind;
  /** Chaves e contas: abrir já no campo de substituir a chave escolhida. */
  substituir?: boolean;
}

/** Uma página de Configurações (`/configuracoes/chaves`, `/configuracoes/sensor-sculk`…). */
export const Route = createFileRoute('/configuracoes/$pagina')({
  validateSearch: (search: Record<string, unknown>): SettingsSearch => ({
    ...(typeof search.chave === 'string' &&
    (SECRET_KINDS as readonly string[]).includes(search.chave)
      ? { chave: search.chave as SecretKind }
      : {}),
    ...(search.substituir === true ? { substituir: true } : {}),
  }),
  beforeLoad: ({ params }) => {
    if (!isSettingsPage(params.pagina)) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- é assim que o TanStack Router recusa a rota
      throw notFound();
    }
  },
  component: SettingsPageRoute,
});

function SettingsPageRoute() {
  const { pagina } = Route.useParams();
  const search = Route.useSearch();
  if (!isSettingsPage(pagina)) return null;
  return <SettingsPageView page={pagina} search={search} />;
}
