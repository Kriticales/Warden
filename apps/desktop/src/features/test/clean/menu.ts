/**
 * O item "Testar como o jogador recebe" do menu ▾ do Testar (grupo "Outros testes"; protótipo
 * `testMenu` em `app.js`). Registrado em `../menu-items.ts`.
 */
import { UserRound } from 'lucide-react';

import i18n from '../../../i18n';
import type { TestMenuItem } from '../menu-items';
import { useTestStore } from '../store';
import { PLAYER_REQUEST } from './api';

const t = i18n.getFixedT(null, 'jogador');

export const playerMenuItem: TestMenuItem = {
  id: 'as-player',
  group: 'other',
  order: 100,
  icon: UserRound,
  label: () => t('menu.item'),
  // Um jogo por vez no Warden inteiro: com qualquer jogo aberto, o item espera.
  description: ({ game }) =>
    game ? i18n.t('menu.comJogoAberto', { ns: 'teste' }) : t('menu.descricao'),
  disabled: ({ game }) => game !== null,
  onSelect: ({ packId, openTest }) => {
    void useTestStore.getState().start(packId, PLAYER_REQUEST);
    openTest();
  },
};
