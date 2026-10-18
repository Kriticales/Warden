/**
 * O servidor local da rodada em andamento (SPEC T25, "O que o Warden está fazendo agora";
 * ROADMAP D-13), pelo `server-state` da L-09: preparando, abrindo, pronto (com a porta) e
 * fechando ao fim da rodada.
 */
import { Server } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import { useServerState } from '../../test/server/api';

export function ServerNow() {
  const { t } = useTranslation('buscaComServidor');
  const server = useServerState();
  let text: string;
  if (!server || server.phase === 'preparing' || server.port === null) {
    text = t('agora.preparando');
  } else if (server.phase === 'starting') {
    text = t('agora.abrindo', { porta: server.port });
  } else if (server.phase === 'ready') {
    text = t('agora.pronto', { porta: server.port });
  } else {
    text = t('agora.parando');
  }
  return (
    <p className="t-sm t-2 mt-2 bisect-server-now">
      <Icon icon={Server} /> {text}
    </p>
  );
}
