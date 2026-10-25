import { createFileRoute, useChildMatches } from '@tanstack/react-router';

import { SculkPage } from '../../../features/diagnostics/ai/SculkPage';
import type { AiSymptom } from '../../../lib/ipc/bindings';

/**
 * Busca da seção: `sessao` começa uma conversa sobre essa sessão do Testar; `assunto` e
 * `sintoma` vêm de "✦ Perguntar ao Sculk" em Problemas (um achado ou o desempenho do último
 * teste) e já preenchem a conversa nova.
 */
interface AiSearch {
  sessao?: string;
  assunto?: string;
  sintoma?: AiSymptom;
}

const SYMPTOMS: readonly AiSymptom[] = ['crashOnStart', 'crashInWorld', 'slow', 'other'];
/** O assunto vai para "Conte o que aconteceu"; um link nunca traz mais que isto. */
const MAX_SUBJECT = 2000;

function validateSearch(search: Record<string, unknown>): AiSearch {
  const result: AiSearch = {};
  if (typeof search.sessao === 'string' && search.sessao !== '') {
    result.sessao = search.sessao;
  }
  if (typeof search.assunto === 'string' && search.assunto.trim() !== '') {
    result.assunto = search.assunto.slice(0, MAX_SUBJECT);
  }
  const sintoma = SYMPTOMS.find((value) => value === search.sintoma);
  if (sintoma) {
    result.sintoma = sintoma;
  }
  return result;
}

/**
 * Seção Sensor Sculk do pack (SPEC T14; ESTRUTURA §13: "Conversar com o Sculk sobre um problema
 * do pack"; protótipo `design/bug-01/sensor-sculk.html`). A conversa aberta é a rota filha
 * (`ia.$conversaId.tsx`), mas a página é uma só: a coluna de conversas fica no lugar ao trocar
 * de conversa. A seção usa a área inteira e o menu do pack fica só com os ícones.
 */
export const Route = createFileRoute('/packs/$packId/ia')({
  validateSearch,
  staticData: { packMenu: 'compact' },
  component: AiRoute,
});

function AiRoute() {
  const { packId } = Route.useParams();
  const { sessao, assunto, sintoma } = Route.useSearch();
  const conversaId = useChildMatches({
    select: (matches) => {
      const params: Partial<Record<string, string>> = matches[0]?.params ?? {};
      return params.conversaId ?? null;
    },
  });
  return (
    <SculkPage
      packId={packId}
      conversationId={conversaId}
      sessionId={conversaId ? null : (sessao ?? null)}
      subject={conversaId ? null : (assunto ?? null)}
      symptom={conversaId ? null : (sintoma ?? null)}
    />
  );
}
