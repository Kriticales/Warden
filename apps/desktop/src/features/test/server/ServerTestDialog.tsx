/**
 * "Testar como servidor…" do menu ▾ (SPEC T27 passos 1 e 2; protótipo `servidor-eula` e
 * `servidor-opcoes`):
 *
 * 1. sem a EULA aceita, o diálogo começa por "Aceitar a EULA do Minecraft?" (com o link
 *    oficial); "Agora não" fecha sem abrir nada; "Aceito a EULA" grava o aceite e segue;
 * 2. as opções: abrir o servidor e entrar nele com o jogo (recomendado) ou só o servidor, a
 *    conta de memória contra a livre agora (com o aviso quando não cabe, CA-T27-03), o que fica
 *    de fora e "Abrir servidor e testar", que começa o teste com o modo `server`. Nada abre sem
 *    esse clique.
 */
import { useNavigate } from '@tanstack/react-router';
import { ExternalLink, Folder, Monitor, Server, Shield } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { openExternal } from '../../../lib/external';
import type { PackId, PackRow } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { Choice } from '../../settings/components/fields';
import { ServerPerfOption } from '../perf-report/ServerPerfOption';
import { useTestStore } from '../store';
import { formatMemory } from '../text';
import { useAcceptEula, useServerPlan } from './api';
import { EULA_URL, clientOnlyText } from './text';

/**
 * Quem abriu o diálogo foi um item do menu ▾, que some ao fechar: o foco volta para o ▾ do
 * Testar no cabeçalho.
 */
function testMenuButton(): HTMLElement | null {
  return document.querySelector<HTMLElement>('header.packhead .testbtn__more');
}

/** O grupo de rádio "Como testar". */
const RADIO_NAME = 'server-test-how';

export interface ServerTestDialogProps {
  packId: PackId;
  pack: PackRow;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ServerTestDialog({ packId, open, onOpenChange }: ServerTestDialogProps) {
  const [withGame, setWithGame] = useState(true);
  const plan = useServerPlan(packId, withGame, open);
  const eulaMissing = plan.isSuccess && plan.data.eulaAcceptedAt === null;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {eulaMissing ? (
        <EulaStep
          onClose={() => {
            onOpenChange(false);
          }}
        />
      ) : (
        <OptionsStep
          packId={packId}
          plan={plan}
          withGame={withGame}
          setWithGame={setWithGame}
          onClose={() => {
            onOpenChange(false);
          }}
        />
      )}
    </Dialog>
  );
}

/** "Aceitar a EULA do Minecraft?" (só na primeira vez; D19). */
function EulaStep({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation('servidor');
  const accept = useAcceptEula();
  return (
    <DialogContent
      size="sm"
      title={t('eula.titulo')}
      returnFocusTo={testMenuButton}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={accept.isPending}>
            {t('eula.agoraNao')}
          </Button>
          <Button
            variant="primary"
            loading={accept.isPending}
            onClick={() => {
              accept.mutate(undefined, {
                onError: (error) => {
                  showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
                },
              });
            }}
          >
            {accept.isPending ? t('eula.aceitando') : t('eula.aceitar')}
          </Button>
        </>
      }
    >
      <p>{t('eula.texto')}</p>
      <p>
        <Button
          variant="link"
          iconEnd={ExternalLink}
          onClick={() => {
            void openExternal(EULA_URL);
          }}
        >
          {t('eula.ler')}
        </Button>
      </p>
      <p className="t-sm t-2">{t('eula.lembrar')}</p>
    </DialogContent>
  );
}

function OptionsStep({
  packId,
  plan,
  withGame,
  setWithGame,
  onClose,
}: {
  packId: PackId;
  plan: ReturnType<typeof useServerPlan>;
  withGame: boolean;
  setWithGame: (value: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation('servidor');
  const start = useTestStore((store) => store.start);
  const navigate = useNavigate();
  // L-12: o perfil de desempenho no servidor (só com o jogo).
  const [performance, setPerformance] = useState(false);
  const data = plan.data;
  const memory = data?.memory;
  const supported = data?.loaderSupported ?? true;

  const open = () => {
    onClose();
    void navigate({ to: '/packs/$packId/teste', params: { packId }, search: {} });
    void start(packId, {
      mode: 'server',
      profile: null,
      replaceInstanceChanges: false,
      serverOnly: !withGame,
      performance: withGame && performance,
    });
  };

  let body;
  if (plan.isError) {
    body = <ErrorPanel error={plan.error} />;
  } else if (!data || !memory) {
    body = <LoadingState label={t('opcoes.carregando')} />;
  } else {
    const sum =
      memory.gameMb === null
        ? t('opcoes.memoriaSo', { servidor: formatMemory(memory.serverMb) })
        : t('opcoes.memoriaConta', {
            servidor: formatMemory(memory.serverMb),
            jogo: formatMemory(memory.gameMb),
            total: formatMemory(memory.totalMb),
          });
    const free =
      memory.freeMb === null ? t('opcoes.livreDesconhecida') : formatMemory(memory.freeMb);
    body = (
      <>
        <fieldset className="m-0 border-0 p-0">
          <legend className="sr-only">{t('opcoes.como')}</legend>
          <div className="choice-list">
            <Choice
              name={RADIO_NAME}
              title={t('opcoes.comJogo')}
              badge={<span className="tag tag--ok">{t('opcoes.recomendado')}</span>}
              desc={t('opcoes.comJogoDesc')}
              checked={withGame}
              onSelect={() => {
                setWithGame(true);
              }}
            />
            <Choice
              name={RADIO_NAME}
              title={t('opcoes.so')}
              desc={t('opcoes.soDesc')}
              checked={!withGame}
              onSelect={() => {
                setWithGame(false);
              }}
            />
          </div>
        </fieldset>
        <ServerPerfOption checked={performance} withGame={withGame} onChange={setPerformance} />
        <dl className="kv">
          <dt>{t('opcoes.memoria')}</dt>
          <dd>{sum}</dd>
          <dt>{t('opcoes.livre')}</dt>
          <dd className={memory.fits ? undefined : 't-warn'}>{free}</dd>
        </dl>
        {memory.fits ? null : (
          <Alert kind="warn" title={t('opcoes.poucaTitulo')}>
            {withGame
              ? t('opcoes.poucaTexto', { livre: free })
              : t('opcoes.poucaTextoSo', { livre: free })}
          </Alert>
        )}
        {supported ? null : <Alert kind="danger">{t('opcoes.loaderNao')}</Alert>}
        <ul className="checklist">
          <li className="is-ok">
            <Icon icon={Shield} />
            <span>{t('opcoes.local')}</span>
          </li>
          <li className="is-ok">
            <Icon icon={Monitor} />
            <span>{clientOnlyText(data.clientOnly, data.clientOnlyCount)}</span>
          </li>
          <li className="is-ok">
            <Icon icon={Folder} />
            <span>{t('opcoes.pasta')}</span>
          </li>
        </ul>
      </>
    );
  }

  return (
    <DialogContent
      title={t('opcoes.titulo')}
      returnFocusTo={testMenuButton}
      description={t('opcoes.sub')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('opcoes.cancelar')}
          </Button>
          <Button variant="primary" icon={Server} disabled={!data || !supported} onClick={open}>
            {t('opcoes.abrir')}
          </Button>
        </>
      }
    >
      {body}
    </DialogContent>
  );
}
