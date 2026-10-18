/**
 * O resultado da busca do culpado (SPEC T25, "Resultado"; protótipo `culpado-resultado` e
 * `culpado~cancelada`):
 *
 * - **Culpado encontrado:** "Culpado encontrado: Epic Fight (junto com o Supplementaries)", a
 *   explicação, a confiança, as rodadas e o tempo, e as ações "Remover … do pack" (confirmação
 *   normal), "Ver o que … altera no jogo" (detalhes do mod, com o raio-x), "Testar de novo" e
 *   "Ver as rodadas". Com outro problema além deste: "Continuar a busca sem …".
 * - **Não reproduziu**, **sem mods já trava**, **Java errado**, **resultados que se
 *   contradizem**, **inconclusiva** e **cancelada** (com o parcial: "O culpado está entre estes
 *   N mods"), cada um com o que fazer.
 */
import { useNavigate } from '@tanstack/react-router';
import { Play, RotateCcw, ScanSearch, Settings, Target, Trash2 } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import type { BisectView, InventoryItem, PackId } from '../../lib/ipc/bindings';
import { useInventory } from '../pack-editor/api';
import { RemoveDialog } from '../pack-editor/mods/RemoveDialog';
import { TestSettingsDialog } from '../pack-editor/test-settings/TestSettingsDialog';
import { usePack } from '../packs/api';
import { formatDuration, formatWhen } from '../test/text';
import { useTestStore } from '../test/store';
import { RoundsTable, RoundsTrack, trackOf } from './Rounds';
import { joinNames, namesOf } from './text';
import { useStartBisect } from './useStartBisect';

export function BisectResult({
  packId,
  view,
  onRestart,
}: {
  packId: PackId;
  view: BisectView;
  onRestart: () => void;
}) {
  const { t } = useTranslation('buscaDoCulpado');
  const outcome = view.outcome;
  const names = namesOf(view.outcomeItems);
  const sub = t('resultado.sub', {
    quando: formatWhen(view.startedAtMs),
    count: view.rounds.length,
    tempo: formatDuration(view.elapsedMs),
  });
  const restart = (
    <Button icon={RotateCcw} onClick={onRestart}>
      {t('resultado.deNovo')}
    </Button>
  );
  const rounds = (
    <details className="disclosure">
      <summary>{t('resultado.verRodadas', { count: view.rounds.length })}</summary>
      <div className="stack-3 mt-2">
        <RoundsTrack entries={trackOf(view, false)} legend={false} />
        <RoundsTable packId={packId} view={view} />
      </div>
    </details>
  );

  if (outcome?.kind === 'found') {
    return <Found packId={packId} view={view} sub={sub} rounds={rounds} />;
  }

  let title: string;
  let body: ReactNode;
  switch (outcome?.kind) {
    case 'notReproduced':
      title = t('resultado.naoReproduziu.titulo');
      body = <Alert kind="neutral">{t('resultado.naoReproduziu.texto')}</Alert>;
      break;
    case 'notAMod':
      title = t('resultado.semMods.titulo');
      body = (
        <Alert kind="warn" actions={<SettingsButton packId={packId} />}>
          {t('resultado.semMods.texto')}
        </Alert>
      );
      break;
    case 'environment':
      title = t('resultado.ambiente.titulo');
      body = (
        <Alert kind="warn" actions={<SettingsButton packId={packId} />}>
          {t('resultado.ambiente.texto')}
        </Alert>
      );
      break;
    case 'contradiction':
      title = t('resultado.contradicao.titulo');
      body = (
        <Alert kind="warn">
          {names.length > 0
            ? t('resultado.contradicao.texto', { count: names.length, nomes: joinNames(names) })
            : t('resultado.contradicao.semNomes')}
        </Alert>
      );
      break;
    case 'inconclusive':
      title = t('resultado.inconclusiva.titulo');
      body = (
        <Alert kind="warn" title={t('resultado.entre', { count: names.length })}>
          <p>{t('resultado.inconclusiva.texto')}</p>
          <p className="t-sm mt-1">{joinNames(names)}</p>
        </Alert>
      );
      break;
    case 'cancelled':
    case undefined:
      title = t('resultado.cancelada.titulo');
      body = (
        <Alert kind="warn" title={t('resultado.entre', { count: names.length })}>
          <p>
            {view.searched > names.length
              ? t('resultado.cancelada.texto', { count: view.searched - names.length })
              : t('resultado.cancelada.nenhum')}
          </p>
          <p className="t-sm mt-1">{joinNames(names)}</p>
        </Alert>
      );
  }
  return (
    <>
      <PageHead title={title} sub={sub} actions={restart} sans />
      <div className="stack bisect-result">
        {body}
        {view.rounds.length > 0 ? rounds : null}
      </div>
    </>
  );
}

function SettingsButton({ packId }: { packId: PackId }) {
  const { t } = useTranslation('buscaDoCulpado');
  const pack = usePack(packId);
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        size="sm"
        icon={Settings}
        onClick={() => {
          setOpen(true);
        }}
      >
        {t('resultado.ajustes')}
      </Button>
      {pack.data ? (
        <TestSettingsDialog packId={packId} pack={pack.data} open={open} onOpenChange={setOpen} />
      ) : null}
    </>
  );
}

function Found({
  packId,
  view,
  sub,
  rounds,
}: {
  packId: PackId;
  view: BisectView;
  sub: string;
  rounds: ReactNode;
}) {
  const { t } = useTranslation('buscaDoCulpado');
  const navigate = useNavigate();
  const inventory = useInventory(packId);
  const start = useStartBisect(packId);
  const startTest = useTestStore((store) => store.start);
  const [removing, setRemoving] = useState<InventoryItem[] | null>(null);
  const outcome = view.outcome?.kind === 'found' ? view.outcome : null;
  const [first, ...partners] = view.outcomeItems;
  if (!outcome || !first) return null;
  const partnerNames = joinNames(namesOf(partners));
  const title =
    partners.length > 0
      ? t('resultado.achou.tituloJunto', { nome: first.name, outros: partnerNames })
      : t('resultado.achou.titulo', { nome: first.name });
  const explanation =
    partners.length > 0
      ? t('resultado.achou.explicacaoJunto', { nome: first.name, outros: partnerNames })
      : t('resultado.achou.explicacao', { nome: first.name });
  const item = inventory.data?.items.find((candidate) => candidate.path === first.path);
  return (
    <>
      <PageHead
        title={
          <>
            <Icon icon={Target} size="lg" className="t-primary" /> {title}
          </>
        }
        sub={sub}
        actions={
          <Button
            icon={Play}
            onClick={() => {
              void navigate({ to: '/packs/$packId/teste', params: { packId }, search: {} });
              void startTest(packId);
            }}
          >
            {t('resultado.testar')}
          </Button>
        }
        sans
      />
      <div className="stack bisect-result">
        <section className="panel panel--strong" aria-labelledby="bisect-result-title">
          <h2 className="sr-only" id="bisect-result-title">
            {t('resultado.secao')}
          </h2>
          <p>{explanation}</p>
          <div className="row row--wrap row--gap-4 mt-3">
            <div>
              <div className="t-caps t-3">{t('resultado.confianca')}</div>
              <div className="mt-1">
                <span className={`status status--${outcome.confidence === 'high' ? 'ok' : 'warn'}`}>
                  {outcome.confidence === 'high'
                    ? t('resultado.alta')
                    : t('resultado.media', { count: view.repetitions })}
                </span>
              </div>
            </div>
            {view.alwaysOn.length > 0 ? (
              <div>
                <div className="t-caps t-3">{t('resultado.tambemLigado')}</div>
                <p className="t-sm">{joinNames(namesOf(view.alwaysOn))}</p>
              </div>
            ) : null}
          </div>
          <div className="btn-row mt-3">
            <Button
              icon={ScanSearch}
              onClick={() => {
                void navigate({
                  to: '/packs/$packId/mods',
                  params: { packId },
                  search: { item: first.path },
                });
              }}
            >
              {t('resultado.altera', { nome: first.name })}
            </Button>
            <Button
              variant="danger-ghost"
              icon={Trash2}
              disabled={!item}
              onClick={() => {
                if (item) setRemoving([item]);
              }}
            >
              {t('resultado.remover', { nome: first.name })}
            </Button>
          </div>
        </section>
        {outcome.anotherProblem ? (
          <Alert
            kind="info"
            title={t('resultado.outro.titulo')}
            actions={
              <Button
                size="sm"
                icon={Target}
                onClick={() => {
                  start({ kind: 'continueWithout' });
                }}
              >
                {t('resultado.outro.continuar', { nome: first.name })}
              </Button>
            }
          >
            {t('resultado.outro.texto', { nome: first.name })}
          </Alert>
        ) : null}
        {view.earlier.map((set) => (
          <p key={set.map((entry) => entry.path).join('|')} className="t-sm t-2">
            {t('resultado.antes', { nomes: joinNames(namesOf(set)) })}
          </p>
        ))}
        {rounds}
        {view.originCrash ? <p className="t-xs t-3">{t('resultado.ligado')}</p> : null}
      </div>
      <RemoveDialog
        packId={packId}
        items={removing}
        onClose={() => {
          setRemoving(null);
        }}
        onRemoved={() => {
          setRemoving(null);
        }}
      />
    </>
  );
}
