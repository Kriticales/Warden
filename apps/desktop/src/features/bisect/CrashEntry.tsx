/**
 * As entradas da busca do culpado a partir de um travamento (SPEC T14 e T25):
 *
 * - [`BisectOffer`]: no resultado "O jogo travou" da tela do teste (registro
 *   `../test/result-blocks.ts`), "Encontrar o mod culpado…" com o travamento de origem;
 * - [`CrashBisectAction`]: em Problemas → Travamentos, na linha de cada causa: "Encontrar o mod
 *   culpado" e, quando já houve uma busca deste travamento, "Ver a busca do culpado".
 *
 * O padrão do "O que acontece" vem do teste: se o jogo chegou a carregar, "Trava ao entrar no
 * mundo"; senão, "Trava ao abrir o jogo".
 */
import { Link } from '@tanstack/react-router';
import { Target } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import type { CrashGroupView, PackId, TestSessionSummary } from '../../lib/ipc/bindings';
import { useCrashes } from '../diagnostics/crashes/api';
import { useGame } from '../test/store';
import { formatWhen } from '../test/text';
import type { ResultBlockProps } from '../test/result-blocks';
import { BisectDialog, type BisectOrigin } from './BisectDialog';

function symptomOf(loaded: boolean): BisectOrigin['symptom'] {
  return loaded ? 'crashOnWorld' : 'crashOnOpen';
}

export function BisectOffer({ packId, session }: ResultBlockProps) {
  if (session.outcome !== 'crashed') {
    return null;
  }
  return <Offer packId={packId} session={session} />;
}

function Offer({ packId, session }: { packId: PackId; session: TestSessionSummary }) {
  const { t } = useTranslation('buscaDoCulpado');
  const crashes = useCrashes(packId);
  const game = useGame();
  const [open, setOpen] = useState(false);
  const group = crashes.data?.groups.find((entry) => entry.latestSessionId === session.id);
  const origin: BisectOrigin = {
    crash: group?.signature.hash ?? null,
    symptom: symptomOf(session.perf?.loadedMs != null),
    hint: t('oferta.igual', { quando: formatWhen(session.startedAtMs) }),
  };
  return (
    <>
      <Alert
        kind="neutral"
        title={t('oferta.titulo')}
        actions={
          <Button
            size="sm"
            variant="primary"
            icon={Target}
            disabled={game !== null}
            onClick={() => {
              setOpen(true);
            }}
          >
            {t('oferta.botao')}
          </Button>
        }
      >
        {group && group.count > 1
          ? t('oferta.textoVezes', { vezes: group.count })
          : t('oferta.texto')}
      </Alert>
      <BisectDialog packId={packId} open={open} onOpenChange={setOpen} origin={origin} />
    </>
  );
}

export function CrashBisectAction({ packId, group }: { packId: PackId; group: CrashGroupView }) {
  const { t } = useTranslation('buscaDoCulpado');
  const game = useGame();
  const [open, setOpen] = useState(false);
  const searched = group.links.culpritSearches.length > 0;
  return (
    <>
      <Button
        size="sm"
        variant="ghost"
        icon={Target}
        disabled={game !== null}
        aria-label={t('travamentos.rotulo', { quando: formatWhen(group.lastAtMs) })}
        onClick={() => {
          setOpen(true);
        }}
      >
        {t('travamentos.encontrar')}
      </Button>
      {searched ? (
        <Button size="sm" variant="ghost" asChild>
          <Link to="/packs/$packId/teste" params={{ packId }} search={{ modo: 'busca' }}>
            {t('travamentos.verBusca')}
          </Link>
        </Button>
      ) : null}
      <BisectDialog
        packId={packId}
        open={open}
        onOpenChange={setOpen}
        origin={{
          crash: group.signature.hash,
          symptom: 'crashOnWorld',
          hint: t('oferta.igual', { quando: formatWhen(group.lastAtMs) }),
        }}
      />
    </>
  );
}
