/**
 * "Como os jogadores instalam (Prism Launcher ou MultiMC)" (SPEC T18; protótipo `publicado`):
 * os quatro passos com o comando do packwiz-installer-bootstrap, a linha de ajuda do erro 403
 * (R5B §11) e "Copiar instruções".
 */
import { Copy } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../components/ui/button';
import { copyText } from './copy';
import { bootstrapCommand, playerInstructions, type GameLabel } from './model';
import './github.css';

export function PlayerSteps({ packLink, game }: { packLink: string; game: GameLabel }) {
  const { t } = useTranslation('publicar');
  const minecraft = game.minecraft ?? '?';
  return (
    <section className="panel" aria-labelledby="passos-jogadores">
      <div className="panel__head">
        <h3 className="panel__title panel__title--sans" id="passos-jogadores">
          {t('passos.titulo')}
        </h3>
        <Button
          size="sm"
          icon={Copy}
          onClick={() => {
            void copyText(t, playerInstructions(t, packLink, game), 'copiar.instrucoesCopiadas');
          }}
        >
          {t('copiar.instrucoes')}
        </Button>
      </div>
      <ol className="howto mt-2">
        <li>
          <span>
            {game.loader
              ? t('passos.criar', { minecraft, loader: game.loader })
              : t('passos.criarSemLoader', { minecraft })}
          </span>
        </li>
        <li>
          <span>{t('passos.baixar')}</span>
        </li>
        <li>
          <span>
            {t('passos.comando')}
            <pre className="code mt-2">{bootstrapCommand(packLink)}</pre>
          </span>
        </li>
        <li>
          <span>{t('passos.abrir')}</span>
        </li>
      </ol>
      <p className="t-xs t-3 mt-2">{t('passos.ajuda403')}</p>
    </section>
  );
}
