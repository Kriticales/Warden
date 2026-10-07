/**
 * Fechar o Warden com o jogo aberto (SPEC T13 "Regras"; CA-T13-07): a janela não fecha sozinha
 * (o Rust segura e emite `game-quit-requested`); aqui o usuário confirma, o Warden para o jogo,
 * grava a sessão e fecha. Montado uma vez na raiz do app, junto com a ligação do `game-state`.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../components/common/ConfirmDialog';
import { events, type GameState } from '../../lib/ipc/bindings';
import { useTauriEvent } from '../../lib/ipc/events';
import { quitWithGame, useGameStateSync } from './api';

export function TestRootHooks() {
  useGameStateSync();
  const { t } = useTranslation('teste');
  const [game, setGame] = useState<GameState | null>(null);
  useTauriEvent(events.gameQuitRequested, (request) => {
    setGame(request.game);
  });
  return (
    <ConfirmDialog
      open={game !== null}
      onOpenChange={(open) => {
        if (!open) setGame(null);
      }}
      title={t('fechar.titulo')}
      description={t('fechar.texto', { pack: game?.packName ?? '' })}
      confirmLabel={t('fechar.confirmar')}
      confirmingLabel={t('fechar.fechando')}
      cancelLabel={t('fechar.continuar')}
      onConfirm={quitWithGame}
    />
  );
}
