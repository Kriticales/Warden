import { Outlet, createFileRoute, useChildMatches } from '@tanstack/react-router';

import { AiPage } from '../../../features/diagnostics/ai/AiPage';
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
/** O assunto vai para "Contar algo"; um link nunca traz mais que isto. */
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
 * Seção ✦ Diagnóstico com IA do pack (SPEC T14; ESTRUTURA §13: "Conversar com a IA sobre um
 * problema do pack"). A conversa aberta é a rota filha (`ia.$conversaId.tsx`).
 */
export const Route = createFileRoute('/packs/$packId/ia')({
  validateSearch,
  component: AiRoute,
});

function AiRoute() {
  const { packId } = Route.useParams();
  const { sessao, assunto, sintoma } = Route.useSearch();
  const children = useChildMatches();
  if (children.length > 0) {
    return <Outlet />;
  }
  return (
    <AiPage
      key={`${sessao ?? ''}|${assunto ?? ''}|${sintoma ?? ''}`}
      packId={packId}
      sessionId={sessao ?? null}
      subject={assunto ?? null}
      symptom={sintoma ?? null}
    />
  );
}
