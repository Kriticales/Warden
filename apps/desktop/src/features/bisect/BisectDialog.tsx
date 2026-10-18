/**
 * Diálogo "Encontrar o mod culpado" (SPEC T25, configuração; protótipo `culpado-config`):
 * o que acontece no jogo, como a busca funciona em linguagem simples, o que fica sempre
 * ligado e o que fica desligado, a estimativa e "Começar a busca".
 *
 * Abre pelo menu ▾ do Testar, pelo resultado "O jogo travou" e por Problemas → Travamentos
 * (com o travamento de origem, que deixa o resultado ligado a ele).
 *
 * Entrar no mundo exige Quick Play (Minecraft 1.20+) e o mundo de teste que o Warden gera sem
 * mods com o servidor do Minecraft (D-12; decisão D18). Na primeira busca do pack (ou depois de
 * mudar a versão), o diálogo explica a geração e pede o aceite da EULA da Mojang, o mesmo do
 * "Testar como servidor" (guardado nas Configurações; aceitar aqui atualiza as duas telas). Sem
 * o aceite, "Trava ao entrar no mundo" fica indisponível e as outras rodadas vão só até o menu.
 * Nas versões anteriores à 1.20, a entrada no mundo é pelo servidor local, que chega com a D-13;
 * até lá a opção explica e fica indisponível.
 */
import { Check, ExternalLink, Play, Target } from 'lucide-react';
import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../components/ui/dialog';
import { Icon } from '../../components/ui/icon';
import { showToast } from '../../components/ui/toast';
import { openExternal } from '../../lib/external';
import type { Symptom, PackId, PackRow } from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { useSettings } from '../settings/api';
import { Choice } from '../settings/components/fields';
import { useAcceptEula } from '../test/server/api';
import { EULA_URL } from '../test/server/text';
import { useBisect, useBisectPlan } from './api';
import { useStartBisect } from './useStartBisect';
import { estimateText, joinNames, namesOf } from './text';

/** De onde a busca começou. */
export interface BisectOrigin {
  /** Hash da assinatura do travamento (Problemas → Travamentos). */
  crash: string | null;
  /** O que acontece no jogo, quando dá para saber pelo travamento. */
  symptom: Symptom['kind'];
  /** Frase que liga a opção ao travamento ("Igual ao travamento de hoje, 14:20"). */
  hint?: string;
}

export interface BisectDialogProps {
  packId: PackId;
  pack?: PackRow;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  origin?: BisectOrigin | undefined;
}

type Kind = Symptom['kind'];

/** Nome do grupo de opções "O que acontece no jogo?". */
const SYMPTOM_FIELD = 'bisect-symptom';

export function BisectDialog({ packId, open, onOpenChange, origin }: BisectDialogProps) {
  // O conteúdo só existe com o diálogo aberto: cada abertura começa da escolha do travamento
  // de origem.
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? <Body packId={packId} onOpenChange={onOpenChange} origin={origin} /> : null}
    </Dialog>
  );
}

function Body({
  packId,
  onOpenChange,
  origin,
}: Pick<BisectDialogProps, 'packId' | 'onOpenChange' | 'origin'>) {
  const { t } = useTranslation('buscaDoCulpado');
  const plan = useBisectPlan(packId, true);
  const saved = useBisect(packId);
  const start = useStartBisect(packId);
  const [kind, setKind] = useState<Kind>(origin?.symptom ?? 'crashOnOpen');
  const [line, setLine] = useState('');
  const lineId = useId();
  const lineHelpId = useId();

  const settings = useSettings();
  const eulaAccepted = (settings.data?.eulaAcceptedAt ?? '').trim() !== '';
  const startRef = useRef<HTMLButtonElement>(null);

  const data = plan.data;
  // O mundo de teste existe para esta versão ou pode ser gerado agora (com a EULA aceita).
  const worldAvailable = data !== undefined && data.quickPlay && (data.worldReady || eulaAccepted);
  const askEula = data !== undefined && data.quickPlay && !data.worldReady && !eulaAccepted;
  const worldBlocked = data
    ? !data.quickPlay
      ? 'servidor'
      : worldAvailable
        ? null
        : 'eula'
    : null;
  const effective: Kind = kind === 'crashOnWorld' && worldBlocked ? 'crashOnOpen' : kind;
  const paused = saved.data?.status === 'paused';
  const canStart =
    plan.isSuccess &&
    data !== undefined &&
    data.searched > 0 &&
    (effective !== 'logLine' || line.trim() !== '');

  const begin = () => {
    const symptom: Symptom =
      effective === 'logLine' ? { kind: 'logLine', text: line.trim() } : { kind: effective };
    onOpenChange(false);
    start({
      kind: 'start',
      request: { symptom, originCrash: origin?.crash ?? null, suspects: [] },
    });
  };

  const choice = (value: Kind, title: string, desc: string, disabled = false) => (
    <Choice
      name={SYMPTOM_FIELD}
      title={title}
      desc={desc}
      checked={effective === value}
      disabled={disabled}
      onSelect={() => {
        setKind(value);
      }}
    />
  );
  const worldDesc = (minecraft: string) => {
    if (worldBlocked === 'servidor') return t('sintoma.mundo.servidor', { minecraft });
    if (worldBlocked === 'eula') return t('sintoma.mundo.semEula');
    return origin?.symptom === 'crashOnWorld' && origin.hint
      ? origin.hint
      : t('sintoma.mundo.desc');
  };
  const openDesc =
    origin?.symptom === 'crashOnOpen' && origin.hint ? origin.hint : t('sintoma.abrir.desc');

  return (
    <DialogContent
      title={t('dialogo.titulo')}
      description={t('dialogo.sub')}
      size="lg"
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('dialogo.cancelar')}</Button>
          </DialogClose>
          <Button
            ref={startRef}
            variant="primary"
            icon={Target}
            disabled={!canStart}
            onClick={begin}
          >
            {t('dialogo.comecar')}
          </Button>
        </>
      }
    >
      {plan.isPending ? <LoadingState label={t('dialogo.carregando')} /> : null}
      {plan.isError ? <ErrorPanel error={plan.error} /> : null}
      {data ? (
        <div className="stack-3">
          <fieldset className="m-0 border-0 p-0">
            <legend className="field__label mb-2">{t('dialogo.oQue')}</legend>
            <div className="choice-list choice-list--2">
              {choice('crashOnOpen', t('sintoma.abrir.titulo'), openDesc)}
              {choice(
                'crashOnWorld',
                t('sintoma.mundo.titulo'),
                worldDesc(data.minecraft),
                worldBlocked !== null,
              )}
              {choice('logLine', t('sintoma.linha.titulo'), t('sintoma.linha.desc'))}
              {choice('assisted', t('sintoma.assistido.titulo'), t('sintoma.assistido.desc'))}
            </div>
          </fieldset>
          {effective === 'logLine' ? (
            <div className="field">
              <label className="field__label" htmlFor={lineId}>
                {t('sintoma.linha.campo')}
              </label>
              <input
                id={lineId}
                className="input"
                value={line}
                aria-describedby={lineHelpId}
                onChange={(event) => {
                  setLine(event.target.value);
                }}
              />
              <p className="field__help" id={lineHelpId}>
                {t('sintoma.linha.ajuda')}
              </p>
            </div>
          ) : null}
          <div className="panel">
            <div className="t-caps t-3">{t('dialogo.estimativa')}</div>
            <div className="t-strong bisect-estimate">{estimateText(data.estimate)}</div>
            <ul className="checklist mt-3">
              <ChecklistItem>
                {data.alwaysOn.length > 0
                  ? `${t('dialogo.entram', { count: data.searched })} ${t('dialogo.sempre', {
                      count: data.alwaysOn.length,
                      sempre: joinNames(namesOf(data.alwaysOn)),
                    })}`
                  : t('dialogo.entram', { count: data.searched })}
              </ChecklistItem>
              <ChecklistItem>{t('dialogo.bibliotecas')}</ChecklistItem>
              <ChecklistItem>
                {data.disabled.length > 0
                  ? t('dialogo.desligados', { nomes: joinNames(namesOf(data.disabled)) })
                  : t('dialogo.semDesligados')}
              </ChecklistItem>
              {worldAvailable && !data.worldReady ? (
                <ChecklistItem>
                  {t('dialogo.criarMundo', { minecraft: data.minecraft })}
                </ChecklistItem>
              ) : null}
              {worldAvailable ? <ChecklistItem>{t('dialogo.mundo')}</ChecklistItem> : null}
            </ul>
            {!data.estimate.measured ? (
              <p className="t-xs t-3 mt-2">{t('dialogo.estimativaSemTeste')}</p>
            ) : null}
          </div>
          {askEula ? (
            <EulaRequest
              onAccepted={() => {
                // O pedido some com o aceite: o foco vai para "Começar a busca".
                startRef.current?.focus();
              }}
            />
          ) : null}
          <p className="t-sm t-2">{t('dialogo.comoFunciona')}</p>
          <p className="t-sm t-2">{t('dialogo.naoMexa')}</p>
          {paused ? (
            <Alert
              kind="warn"
              compact
              title={t('dialogo.pausadaTitulo')}
              actions={
                <Button
                  size="sm"
                  icon={Play}
                  onClick={() => {
                    onOpenChange(false);
                    start({ kind: 'resume' });
                  }}
                >
                  {t('dialogo.continuarPausada')}
                </Button>
              }
            >
              {t('dialogo.pausadaTexto')}
            </Alert>
          ) : null}
          {data.searched === 0 ? <Alert kind="warn" compact title={t('dialogo.semMods')} /> : null}
        </div>
      ) : null}
    </DialogContent>
  );
}

/**
 * "Para entrar no mundo, o Warden cria um mundo de teste": o que a geração faz, o link da EULA
 * e "Aceito a EULA" (D18).
 */
function EulaRequest({ onAccepted }: { onAccepted: () => void }) {
  const { t } = useTranslation('buscaDoCulpado');
  const accept = useAcceptEula();
  return (
    <Alert
      kind="info"
      title={t('eula.titulo')}
      actionsBelow
      actions={
        <>
          <Button
            size="sm"
            variant="primary"
            loading={accept.isPending}
            onClick={() => {
              accept.mutate(undefined, {
                onSuccess: onAccepted,
                onError: (error) => {
                  showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
                },
              });
            }}
          >
            {accept.isPending ? t('eula.aceitando') : t('eula.aceitar')}
          </Button>
          <Button
            size="sm"
            variant="link"
            iconEnd={ExternalLink}
            onClick={() => {
              void openExternal(EULA_URL);
            }}
          >
            {t('eula.ler')}
          </Button>
        </>
      }
    >
      <p>{t('eula.texto')}</p>
      <p className="t-sm t-2 mt-2">{t('eula.semAceite')}</p>
    </Alert>
  );
}

function ChecklistItem({ children }: { children: string }) {
  return (
    <li className="is-ok">
      <Icon icon={Check} />
      <span>{children}</span>
    </li>
  );
}
