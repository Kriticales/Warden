/**
 * Diálogo "Encontrar o mod culpado" (SPEC T25, configuração; protótipo `culpado-config`):
 * o que acontece no jogo, como a busca funciona em linguagem simples, o que fica sempre
 * ligado e o que fica desligado, a estimativa e "Começar a busca".
 *
 * Abre pelo menu ▾ do Testar, pelo resultado "O jogo travou" e por Problemas → Travamentos
 * (com o travamento de origem, que deixa o resultado ligado a ele).
 *
 * "Trava ao entrar no mundo" precisa entrar no mundo sozinho: Quick Play (Minecraft 1.20+) e
 * um mundo de teste na instância de teste. Nas versões anteriores, e quando o travamento foi no
 * servidor, a entrada no mundo é pelo servidor local (D-13): o diálogo explica e pede antes
 * ([`ServerConsent`]); sem o aceite, a busca usa o modo assistido.
 */
import { Check, Play, Target } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../components/ui/dialog';
import { Icon } from '../../components/ui/icon';
import type { Symptom, PackId, PackRow } from '../../lib/ipc/bindings';
import { Choice } from '../settings/components/fields';
import { useBisect, useBisectPlan } from './api';
import { serverReason, withServer } from './server/need';
import { ServerConsent } from './server/ServerConsent';
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
  /** O travamento foi no servidor (D-13): a busca pede o servidor local. */
  server?: boolean;
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
  const { t: ts } = useTranslation('buscaComServidor');
  const plan = useBisectPlan(packId, true);
  const saved = useBisect(packId);
  const start = useStartBisect(packId);
  const [kind, setKind] = useState<Kind>(origin?.symptom ?? 'crashOnOpen');
  const [line, setLine] = useState('');
  const [serverAccepted, setServerAccepted] = useState(false);
  const lineId = useId();
  const lineHelpId = useId();

  const data = plan.data;
  const fromServer = origin?.server === true;
  // Sem Quick Play (ou com o travamento no servidor), quem entra no mundo é o servidor local, que
  // cria o mundo quando a instância de teste não tem um.
  const worldBlocked = data && data.quickPlay && !data.world && !fromServer ? 'mundo' : null;
  const effective: Kind = kind === 'crashOnWorld' && worldBlocked ? 'crashOnOpen' : kind;
  const reason = data ? serverReason(data, effective, fromServer) : null;
  const paused = saved.data?.status === 'paused';
  const canStart =
    plan.isSuccess &&
    data !== undefined &&
    data.searched > 0 &&
    (effective !== 'logLine' || line.trim() !== '');

  const begin = () => {
    const chosen: Symptom =
      effective === 'logLine' ? { kind: 'logLine', text: line.trim() } : { kind: effective };
    const { symptom, server } = withServer(chosen, reason, serverAccepted);
    onOpenChange(false);
    start({
      kind: 'start',
      request: { symptom, originCrash: origin?.crash ?? null, suspects: [], server },
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
  const worldDesc = (minecraft: string, quickPlay: boolean) => {
    if (!quickPlay) return ts('sintoma.servidor', { minecraft });
    if (worldBlocked === 'mundo') return t('sintoma.mundo.semMundo');
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
          <Button variant="primary" icon={Target} disabled={!canStart} onClick={begin}>
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
                worldDesc(data.minecraft, data.quickPlay),
                worldBlocked !== null,
              )}
              {choice('logLine', t('sintoma.linha.titulo'), t('sintoma.linha.desc'))}
              {choice('assisted', t('sintoma.assistido.titulo'), t('sintoma.assistido.desc'))}
            </div>
          </fieldset>
          {reason ? (
            <ServerConsent
              packId={packId}
              reason={reason}
              minecraft={data.minecraft}
              hasWorld={data.world !== null}
              accepted={serverAccepted}
              onAcceptedChange={setServerAccepted}
            />
          ) : null}
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
              {data.world && data.quickPlay ? (
                <ChecklistItem>{t('dialogo.mundo', { mundo: data.world })}</ChecklistItem>
              ) : null}
            </ul>
            {!data.estimate.measured ? (
              <p className="t-xs t-3 mt-2">{t('dialogo.estimativaSemTeste')}</p>
            ) : null}
          </div>
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

function ChecklistItem({ children }: { children: string }) {
  return (
    <li className="is-ok">
      <Icon icon={Check} />
      <span>{children}</span>
    </li>
  );
}
