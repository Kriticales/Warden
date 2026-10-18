/**
 * O resultado do "Testar como servidor" (SPEC T27 passo 4; CA-T27-04), no registro
 * `result-blocks.ts`: o que aconteceu em cada lado ("O servidor abriu em 14 s e fechou
 * normalmente; o jogo entrou no servidor") e, quando o servidor travou, "Por que o servidor
 * travou", com o mod apontado e o trecho do log do servidor. Numa sessão com o jogo, dá para
 * abrir o console do servidor gravado.
 */
import { Server, Terminal } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { ServerCrash } from '../../../lib/ipc/bindings';
import { Console } from '../console/Console';
import type { ResultBlockProps } from '../result-blocks';
import { useServerSessionLog } from './api';
import { gameSideText, serverSideText } from './text';

export function ServerResult({ packId, session }: ResultBlockProps) {
  const { t } = useTranslation('servidor');
  const [showConsole, setShowConsole] = useState(false);
  const side = session.server;
  const log = useServerSessionLog(packId, session.id, showConsole && side !== null && !side.only);
  if (session.mode !== 'server' || !side) {
    return null;
  }
  return (
    <section className="panel stack-3" aria-labelledby="server-result-title">
      <div className="row">
        <h2 className="panel__title panel__title--sans grow" id="server-result-title">
          <Icon icon={Server} /> {t('resultado.lados')}
        </h2>
        {side.only ? null : (
          <Button
            variant="ghost"
            size="sm"
            icon={Terminal}
            aria-expanded={showConsole}
            onClick={() => {
              setShowConsole((open) => !open);
            }}
          >
            {showConsole ? t('resultado.ocultarConsole') : t('resultado.verConsole')}
          </Button>
        )}
      </div>
      <dl className="kv">
        <dt>{t('resultado.servidor')}</dt>
        <dd>{serverSideText(side)}</dd>
        <dt>{t('resultado.jogo')}</dt>
        <dd>{gameSideText(side)}</dd>
      </dl>
      {side.crash ? <WhyServerCrashed crash={side.crash} /> : null}
      {showConsole ? (
        log.isError ? (
          <ErrorPanel error={log.error} />
        ) : log.data ? (
          <div className="result-console">
            <Console lines={log.data.lines} state="ended" label={t('jogo.consoleServidor')} />
          </div>
        ) : (
          <LoadingState label={t('resultado.carregandoConsole')} />
        )
      ) : null}
    </section>
  );
}

/** "Por que o servidor travou" (também no resultado do pacote para servidor, E-03). */
export function WhyServerCrashed({ crash }: { crash: ServerCrash }) {
  const { t } = useTranslation('servidor');
  const [mod] = crash.mods;
  let cause: string;
  if (mod && crash.clientClass) {
    cause = t('resultado.modCliente', { mod: mod.name });
  } else if (mod) {
    cause = t('resultado.modCulpado', { mod: mod.name });
  } else {
    cause = t('resultado.semCausa');
  }
  return (
    <div className="stack-2" aria-labelledby="server-crash-title">
      <h3 className="t-strong" id="server-crash-title">
        {t('resultado.porQue')}
      </h3>
      <p>{cause}</p>
      {crash.clientClass ? (
        <p className="t-sm t-2">
          {t('resultado.classe', { classe: crash.clientClass })}{' '}
          {mod ? t('resultado.marcarCliente') : null}
        </p>
      ) : null}
      {crash.excerpt.length > 0 ? (
        <>
          <h4 className="t-sm t-2">{t('resultado.trecho')}</h4>
          <pre className="errpanel__pre">{crash.excerpt.join('\n')}</pre>
        </>
      ) : null}
    </div>
  );
}
