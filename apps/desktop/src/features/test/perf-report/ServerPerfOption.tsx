/**
 * A opção "Medir o desempenho do servidor com o spark" do "Testar como servidor…" (SPEC T13,
 * "No servidor local, o Warden liga e desliga o spark sozinho pelo console do servidor"; ROADMAP
 * L-12). Desenhada no diálogo da L-09; só vale com o jogo aberto junto (o Warden liga o spark
 * quando o jogo abre).
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

export function ServerPerfOption({
  checked,
  withGame,
  onChange,
}: {
  checked: boolean;
  /** "Abrir o servidor e entrar com o jogo" (sem isso, a opção fica desligada). */
  withGame: boolean;
  onChange: (checked: boolean) => void;
}) {
  const { t } = useTranslation('perfilDesempenho');
  const descId = useId();
  return (
    <div className="drow drow--compact drow--nobox">
      <span className="drow__box">
        <label className="check">
          <input
            type="checkbox"
            checked={checked && withGame}
            disabled={!withGame}
            aria-label={t('servidor.opcao')}
            aria-describedby={descId}
            onChange={(event) => {
              onChange(event.target.checked);
            }}
          />
        </label>
      </span>
      <span className="drow__main">
        <span className="drow__line">
          <b>{t('servidor.opcao')}</b>
        </span>
        <span className="drow__desc" id={descId}>
          {withGame ? t('servidor.opcaoDesc') : t('servidor.soComJogo')}
        </span>
      </span>
    </div>
  );
}
