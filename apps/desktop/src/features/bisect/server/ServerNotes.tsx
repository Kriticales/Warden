/**
 * O servidor local no resultado e nas rodadas da busca do culpado (SPEC T25; ROADMAP D-13;
 * CA-T25-06): "no servidor" nas rodadas que travaram nele e, no resultado, que cada rodada
 * abriu o servidor, quantas travaram nele e que o log de cada rodada tem os dois lados.
 */
import { useTranslation } from 'react-i18next';

import type { BisectView } from '../../../lib/ipc/bindings';

/** "no servidor", ao lado do resultado de uma rodada que travou no servidor. */
export function OnServerTag() {
  const { t } = useTranslation('buscaComServidor');
  return <span className="t-xs t-3"> · {t('rodada.noServidor')}</span>;
}

/** No resultado de uma busca com o servidor local. */
export function ServerResultNote({ view }: { view: BisectView }) {
  const { t } = useTranslation('buscaComServidor');
  if (!view.server) return null;
  const count = view.rounds.filter((round) => round.serverFailed).length;
  return (
    <p className="t-sm t-2">
      <span className="t-strong">{t('resultado.titulo')}:</span>{' '}
      {count > 0 ? t('resultado.texto', { count }) : t('resultado.semFalha')}
    </p>
  );
}
