/**
 * "Como o jogador recebe" no resultado do teste (SPEC T13; ROADMAP L-07): de onde o pack veio,
 * o aviso de versão anterior do GitHub, os mods que não baixam sozinhos para os jogadores, o
 * Crash Assistant e a tabela do que o jogador recebeu de diferente do pack, em linguagem
 * simples. Registrado em `../result-blocks.ts`; só aparece nas sessões desse modo.
 */
import { RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { PlayerItem, PlayerReport } from '../../../lib/ipc/bindings';
import type { ResultBlockProps } from '../result-blocks';
import { usePlayerReport, useTestAsPlayer } from './api';

export function PlayerResult({ packId, session }: ResultBlockProps) {
  if (session.mode !== 'asPlayer') {
    return null;
  }
  return <PlayerResultBody packId={packId} sessionId={session.id} />;
}

function PlayerResultBody({
  packId,
  sessionId,
}: {
  packId: ResultBlockProps['packId'];
  sessionId: string;
}) {
  const { t } = useTranslation('jogador');
  const report = usePlayerReport(packId, sessionId);
  const testAgain = useTestAsPlayer(packId);

  if (report.isPending) {
    return <LoadingState inline label={t('resultado.carregando')} />;
  }
  if (report.isError) {
    return <ErrorPanel compact error={report.error} />;
  }
  if (!report.data) {
    return null;
  }
  return <PlayerReportPanel report={report.data} onTestAgain={testAgain} />;
}

/** O resultado gravado (sem consulta: também usado nos testes de componente). */
export function PlayerReportPanel({
  report,
  onTestAgain,
}: {
  report: PlayerReport;
  onTestAgain: () => void;
}) {
  const { t } = useTranslation('jogador');
  const blocked = report.items.filter((item) => item.kind === 'blocked').length;
  return (
    <section className="panel stack-3" aria-labelledby="player-result-title">
      <h2 className="panel__title" id="player-result-title">
        {t('resultado.titulo')}
      </h2>
      <p>
        {report.source.kind === 'published'
          ? t('resultado.origemPublicado', { versao: report.source.version ?? '' })
          : t('resultado.origemLocal')}{' '}
        {t('resultado.recebidos', { count: report.receivedFiles })}
      </p>
      {report.outdated ? (
        <Alert
          kind="warn"
          actions={
            <Button icon={RotateCcw} onClick={onTestAgain}>
              {t('resultado.tentarDeNovo')}
            </Button>
          }
        >
          {t('resultado.versaoAnterior')}
        </Alert>
      ) : null}
      {blocked > 0 ? (
        <Alert kind="warn" title={t('resultado.bloqueados', { count: blocked })}>
          {t('resultado.bloqueadosTexto')}
        </Alert>
      ) : null}
      {report.crashAssistant === null ? null : (
        <p className="t-sm t-2">
          <span className={report.crashAssistant ? 'status status--ok' : 'status status--warn'}>
            {report.crashAssistant
              ? t('resultado.crashAssistantChegou')
              : t('resultado.crashAssistantFaltou')}
          </span>
        </p>
      )}
      {report.items.length === 0 ? (
        <Alert kind="ok" compact role="status">
          {t('resultado.igual')}
        </Alert>
      ) : (
        <div className="tablewrap">
          <table className="table" aria-label={t('resultado.tabela.rotulo')}>
            <thead>
              <tr>
                <th scope="col">{t('resultado.tabela.item')}</th>
                <th scope="col">{t('resultado.tabela.arquivo')}</th>
                <th scope="col">{t('resultado.tabela.oQue')}</th>
              </tr>
            </thead>
            <tbody>
              {report.items.map((item) => (
                <tr key={`${item.kind}:${item.path}`}>
                  <td className="t-strong">{item.name}</td>
                  <td className="path">{item.path}</td>
                  <td>
                    <span className={`status status--${tone(item)}`}>{kindText(item, t)}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

/** Cor da situação: o que falta para o jogador é aviso; o resto, discreto. */
function tone(item: PlayerItem): 'warn' | 'muted' {
  return item.kind === 'optional' || item.kind === 'extra' ? 'muted' : 'warn';
}

function kindText(item: PlayerItem, t: ReturnType<typeof useTranslation<'jogador'>>['t']): string {
  switch (item.kind) {
    case 'blocked':
      return t('resultado.tipo.blocked');
    case 'notDownloaded':
      return t('resultado.tipo.notDownloaded');
    case 'notPublished':
      return t('resultado.tipo.notPublished');
    case 'serverOnly':
      return t('resultado.tipo.serverOnly');
    case 'optional':
      return item.defaultOn === false
        ? t('resultado.tipo.optionalOff')
        : t('resultado.tipo.optionalOn');
    case 'extra':
      return t('resultado.tipo.extra');
  }
}
