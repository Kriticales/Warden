/**
 * O item "Testar com perfil de desempenho" do menu ▾ do Testar (grupo "Outros testes";
 * protótipo `testMenu` em `app.js`). Registrado em `../menu-items.ts`.
 */
import { Gauge } from 'lucide-react';

import i18n from '../../../i18n';
import type { TestMenuItem } from '../menu-items';
import { useTestStore } from '../store';
import { PERF_REQUEST } from './api';

const t = i18n.getFixedT(null, 'perfilDesempenho');

export const perfMenuItem: TestMenuItem = {
  id: 'perf-profile',
  group: 'other',
  order: 300,
  icon: Gauge,
  label: () => t('menu.item'),
  // Um jogo por vez no Warden inteiro: com qualquer jogo aberto, o item espera.
  description: ({ game }) =>
    game ? i18n.t('menu.comJogoAberto', { ns: 'teste' }) : t('menu.descricao'),
  disabled: ({ game }) => game !== null,
  onSelect: ({ packId, openTest }) => {
    void useTestStore.getState().start(packId, PERF_REQUEST);
    openTest();
  },
};
