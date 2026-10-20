/**
 * Os mods embutidos de um item, para ler (P2): o nome que cada jar declara, com os módulos da
 * Fabric API juntos ("partes da Fabric API (N)"). O id fica só no `title`, para quem quiser
 * conferir; o texto não mostra `fabric_api_base` cru.
 */
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { EmbeddedMod } from '../../../lib/ipc/bindings';
import { embeddedEntries } from './model';

export function EmbeddedNames({ embedded }: { embedded: readonly EmbeddedMod[] }) {
  const { t } = useTranslation('grafoDesenho');
  const entries = embeddedEntries(embedded);
  const texts = entries.map((entry) =>
    entry.fabricParts > 0 ? t('no.partesFabric', { count: entry.fabricParts }) : entry.name,
  );
  // As vírgulas e o "e" vêm do Intl, como no resto do grafo; cada nome vira um elemento.
  const parts = new Intl.ListFormat('pt-BR', { type: 'conjunction' }).formatToParts(texts);
  const nodes: ReactNode[] = [];
  let index = 0;
  for (const part of parts) {
    if (part.type === 'literal') {
      nodes.push(part.value);
      continue;
    }
    const entry = entries[index];
    nodes.push(
      <span key={entry?.key ?? index} title={entry?.ids.join(', ')}>
        {part.value}
      </span>,
    );
    index += 1;
  }
  return <>{nodes}</>;
}
