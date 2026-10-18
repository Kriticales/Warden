/**
 * O pedido do servidor local no diálogo "Encontrar o mod culpado" (SPEC T25 e T27; decisão
 * D18; ROADMAP D-13; protótipo `culpado-config~velho`): quando a entrada no mundo exige o
 * servidor (Minecraft anterior a 1.20) ou o travamento foi no servidor, o Warden explica por
 * que precisa abrir um servidor neste computador, mostra a memória (servidor + jogo contra a
 * livre agora, com o aviso quando não cabe), o que fica fora do servidor e pede a EULA se ainda
 * não foi aceita. Nada abre sem "Entendi, pode abrir"; sem o aceite, a busca usa o modo
 * assistido (quem decide é o diálogo).
 *
 * A ação é um botão só, no mesmo lugar ("Aceito a EULA" → "Entendi, pode abrir" → "Não abrir o
 * servidor"): o foco fica nele a cada passo.
 */
import { ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { openExternal } from '../../../lib/external';
import type { PackId } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useAcceptEula, useServerPlan } from '../../test/server/api';
import { EULA_URL } from '../../test/server/text';
import { formatMemory } from '../../test/text';
import type { ServerReason } from './need';

export interface ServerConsentProps {
  packId: PackId;
  /** Por que a busca precisa do servidor. */
  reason: ServerReason;
  /** Versão do Minecraft do pack. */
  minecraft: string;
  /** A instância de teste tem mundo de teste (senão o servidor cria um em cada rodada). */
  hasWorld: boolean;
  /** "Entendi, pode abrir" foi escolhido. */
  accepted: boolean;
  onAcceptedChange: (accepted: boolean) => void;
}

export function ServerConsent({
  packId,
  reason,
  minecraft,
  hasWorld,
  accepted,
  onAcceptedChange,
}: ServerConsentProps) {
  const { t } = useTranslation('buscaComServidor');
  const { t: ts } = useTranslation('servidor');
  const plan = useServerPlan(packId, true, true);
  const accept = useAcceptEula();

  if (plan.isError) {
    return <ErrorPanel error={plan.error} />;
  }
  if (!plan.data) {
    return <LoadingState label={ts('opcoes.carregando')} />;
  }
  const { memory, eulaAcceptedAt, loaderSupported, clientOnlyCount } = plan.data;
  const free =
    memory.freeMb === null ? ts('opcoes.livreDesconhecida') : formatMemory(memory.freeMb);
  const total =
    memory.gameMb === null
      ? ts('opcoes.memoriaSo', { servidor: formatMemory(memory.serverMb) })
      : ts('opcoes.memoriaConta', {
          servidor: formatMemory(memory.serverMb),
          jogo: formatMemory(memory.gameMb),
          total: formatMemory(memory.totalMb),
        });
  const eulaMissing = eulaAcceptedAt === null;

  let action = null;
  if (loaderSupported) {
    if (eulaMissing) {
      action = (
        <Button
          size="sm"
          variant="primary"
          aria-busy={accept.isPending || undefined}
          onClick={() => {
            if (accept.isPending) return;
            accept.mutate(undefined, {
              onError: (error) => {
                showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
              },
            });
          }}
        >
          {accept.isPending ? ts('eula.aceitando') : ts('eula.aceitar')}
        </Button>
      );
    } else {
      action = (
        <Button
          size="sm"
          variant={accepted ? 'ghost' : 'primary'}
          onClick={() => {
            onAcceptedChange(!accepted);
          }}
        >
          {accepted ? t('pedido.naoAbrir') : t('pedido.pode')}
        </Button>
      );
    }
  }

  return (
    <Alert
      kind={accepted && !eulaMissing ? 'ok' : 'info'}
      title={t('pedido.titulo')}
      actions={action}
      actionsBelow
    >
      <div className="stack-2">
        <p>{reason === 'server' ? t('pedido.servidor') : t('pedido.versao', { minecraft })}</p>
        {hasWorld ? null : <p className="t-sm t-2">{t('pedido.semMundo')}</p>}
        <dl className="kv">
          <dt>{ts('opcoes.memoria')}</dt>
          <dd>{total}</dd>
          <dt>{ts('opcoes.livre')}</dt>
          <dd className={memory.fits ? undefined : 't-warn'}>{free}</dd>
        </dl>
        {memory.fits ? null : (
          <Alert kind="warn" compact title={ts('opcoes.poucaTitulo')}>
            {t('pedido.pouca', { livre: free })}
          </Alert>
        )}
        {clientOnlyCount > 0 ? (
          <p className="t-sm t-2">{t('pedido.soCliente', { count: clientOnlyCount })}</p>
        ) : null}
        {loaderSupported ? null : (
          <Alert kind="warn" compact>
            {ts('opcoes.loaderNao')}
          </Alert>
        )}
        {loaderSupported && eulaMissing ? (
          <div>
            <p className="t-sm">{ts('eula.texto')}</p>
            <p className="mt-1">
              <Button
                variant="link"
                iconEnd={ExternalLink}
                onClick={() => {
                  void openExternal(EULA_URL);
                }}
              >
                {ts('eula.ler')}
              </Button>
            </p>
          </div>
        ) : null}
        <p className="t-sm t-2" role="status">
          {accepted && !eulaMissing ? t('pedido.aceito') : t('pedido.semAceite')}
        </p>
      </div>
    </Alert>
  );
}
