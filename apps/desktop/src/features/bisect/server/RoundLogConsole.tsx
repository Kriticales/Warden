/**
 * "Ver log" de uma rodada com o servidor local (SPEC T25 e T27; ROADMAP D-13): o log do jogo e
 * o do servidor, um de cada vez, com "Mostrando: Servidor · Jogo" (caixa de seleção, não aba,
 * como no "Testar como servidor"). Sem linhas do servidor, só o console do jogo.
 */
import { useId, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ConsoleLine } from '../../../lib/ipc/bindings';
import { Console } from '../../test/console/Console';
import { splitLines } from '../../test/server/ServerRunningView';
import './bisect-server.css';

type Shown = 'server' | 'game';

/** As opções de "Mostrando": valor e texto do catálogo `servidor`. */
const SHOWN_OPTIONS = [
  ['server', 'jogo.servidor'],
  ['game', 'jogo.jogo'],
] as const satisfies readonly (readonly [Shown, string])[];

export function RoundLogConsole({
  lines,
  label,
  className,
}: {
  lines: readonly ConsoleLine[];
  label: string;
  className: string;
}) {
  const { t } = useTranslation('servidor');
  const split = useMemo(() => splitLines(lines), [lines]);
  // A falha costuma estar no servidor quando ele tem log; o jogo pode nem ter aberto.
  const [shown, setShown] = useState<Shown>(split.game.length > 0 ? 'game' : 'server');
  const selectId = useId();
  if (split.server.length === 0) {
    return <Console lines={lines} state="ended" label={label} className={className} />;
  }
  const showServer = shown === 'server';
  return (
    <div className="stack-2">
      <div className="row bisect-server-log__show">
        <label className="t-sm t-2" htmlFor={selectId}>
          {t('jogo.mostrando')}
        </label>
        <select
          id={selectId}
          className="select select--sm"
          value={shown}
          onChange={(event) => {
            setShown(event.target.value as Shown);
          }}
        >
          {SHOWN_OPTIONS.map(([value, key]) => (
            <option key={value} value={value}>
              {t(key)}
            </option>
          ))}
        </select>
      </div>
      <Console
        key={shown}
        lines={showServer ? split.server : split.game}
        state="ended"
        label={showServer ? t('jogo.consoleServidor') : t('jogo.consoleJogo')}
        className={className}
      />
    </div>
  );
}
