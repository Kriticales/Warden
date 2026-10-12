/**
 * Itens do menu ▾ do Testar (SPEC T13; ESTRUTURA N7). **Registro acréscimo-apenas** (ROADMAP
 * §1): as tarefas da D4 acrescentam uma linha cada — L-07 ("Testar como o jogador recebe"),
 * L-09 ("Testar como servidor…"), L-12 ("Testar com perfil de desempenho"), D-12 ("Encontrar o
 * mod culpado…") em `other`; L-08 (os perfis) em `profile`.
 *
 * Grupos, nesta ordem, cada um com o título do catálogo (grupo sem itens não aparece):
 * - `last`: "Ver último teste" (sem título);
 * - `other`: "Outros testes";
 * - `profile`: "Perfil do teste" (com "Ajustes do teste neste computador…", da P1-08);
 * - `instance`: "Instância de teste".
 *
 * Um item que abre um diálogo pede `context.openDialog(id)`; o diálogo fica em `testDialogs`
 * (também acréscimo-apenas), fora do menu, para continuar aberto depois que o menu fecha.
 */
import { FolderOpen, History, RotateCcw, Settings, Trash2, type LucideIcon } from 'lucide-react';
import type { ComponentType } from 'react';

import i18n from '../../i18n';
import type { GameState, PackId, PackRow, TestSessionSummary } from '../../lib/ipc/bindings';
import { TestSettingsDialog } from '../pack-editor/test-settings/TestSettingsDialog';
import { DeleteWorldsDialog } from './dialogs/DeleteWorldsDialog';
import { RecreateInstanceDialog } from './dialogs/RecreateInstanceDialog';
import { formatWhen, outcomeShort } from './text';

export type TestMenuGroup = 'last' | 'other' | 'profile' | 'instance';

/** Os grupos na ordem do menu, com a chave do título (`null` = sem título). */
export const TEST_MENU_GROUPS: readonly { id: TestMenuGroup; titleKey: string | null }[] = [
  { id: 'last', titleKey: null },
  { id: 'other', titleKey: 'menu.outrosTestes' },
  { id: 'profile', titleKey: 'menu.perfil' },
  { id: 'instance', titleKey: 'menu.instancia' },
];

/** O que cada item sabe na hora de aparecer. */
export interface TestMenuContext {
  packId: PackId;
  pack: PackRow;
  /** O jogo aberto no Warden (deste ou de outro pack). */
  game: GameState | null;
  /** A sessão mais nova do pack. */
  lastSession: TestSessionSummary | null;
  /** Se a instância de teste já existe. */
  instanceExists: boolean;
  /** Vai para a tela do teste (de uma sessão gravada, com `sessionId`). */
  openTest: (sessionId?: string) => void;
  /** Abre um diálogo de `testDialogs`. */
  openDialog: (id: string) => void;
  /** "Abrir pasta da instância de teste". */
  revealInstance: () => void;
}

export interface TestMenuItem {
  /** Identificador estável. */
  id: string;
  group: TestMenuGroup;
  /** Posição dentro do grupo (crescente). */
  order: number;
  icon: LucideIcon;
  label: (context: TestMenuContext) => string;
  /** Linha pequena abaixo do texto. */
  description?: (context: TestMenuContext) => string | null;
  disabled?: (context: TestMenuContext) => boolean;
  onSelect: (context: TestMenuContext) => void;
}

/** Um diálogo aberto por um item do menu. */
export interface TestDialog {
  id: string;
  component: ComponentType<{
    packId: PackId;
    pack: PackRow;
    open: boolean;
    onOpenChange: (open: boolean) => void;
  }>;
}

const t = i18n.getFixedT(null, 'teste');

/** Se o jogo deste pack está aberto (ou em preparação). */
function gameHere(context: TestMenuContext): boolean {
  return context.game?.packId === context.packId;
}

/** Registro acréscimo-apenas: uma linha por item. */
export const testMenuItems: readonly TestMenuItem[] = [
  {
    id: 'last-test',
    group: 'last',
    order: 100,
    icon: History,
    label: () => t('menu.verUltimo'),
    description: ({ lastSession }) =>
      lastSession
        ? `${formatWhen(lastSession.startedAtMs)} · ${outcomeShort(lastSession.outcome)}`
        : t('menu.verUltimoNunca'),
    disabled: ({ lastSession }) => lastSession === null,
    onSelect: ({ lastSession, openTest }) => {
      if (lastSession) openTest(lastSession.id);
    },
  },
  {
    id: 'settings',
    group: 'profile',
    order: 900,
    icon: Settings,
    label: () => i18n.t('cabecalho.ajustesDoTeste', { ns: 'editor' }),
    description: () => i18n.t('cabecalho.ajustesDoTesteDesc', { ns: 'editor' }),
    onSelect: ({ openDialog }) => {
      openDialog('settings');
    },
  },
  {
    id: 'reveal-instance',
    group: 'instance',
    order: 100,
    icon: FolderOpen,
    label: () => t('menu.abrirPasta'),
    description: ({ instanceExists }) => (instanceExists ? null : t('menu.semInstancia')),
    disabled: ({ instanceExists }) => !instanceExists,
    onSelect: ({ revealInstance }) => {
      revealInstance();
    },
  },
  {
    id: 'delete-worlds',
    group: 'instance',
    order: 200,
    icon: Trash2,
    label: () => t('menu.apagarMundos'),
    description: (context) => (gameHere(context) ? t('menu.comJogoAberto') : null),
    disabled: (context) => !context.instanceExists || gameHere(context),
    onSelect: ({ openDialog }) => {
      openDialog('delete-worlds');
    },
  },
  {
    id: 'recreate-instance',
    group: 'instance',
    order: 300,
    icon: RotateCcw,
    label: () => t('menu.recriar'),
    description: (context) => (gameHere(context) ? t('menu.comJogoAberto') : null),
    disabled: (context) => !context.instanceExists || gameHere(context),
    onSelect: ({ openDialog }) => {
      openDialog('recreate-instance');
    },
  },
];

/** Registro acréscimo-apenas: um diálogo por linha. */
export const testDialogs: readonly TestDialog[] = [
  { id: 'settings', component: TestSettingsDialog },
  { id: 'delete-worlds', component: DeleteWorldsDialog },
  { id: 'recreate-instance', component: RecreateInstanceDialog },
];

/** Os itens de um grupo, na ordem. */
export function itemsOf(
  group: TestMenuGroup,
  items: readonly TestMenuItem[] = testMenuItems,
): TestMenuItem[] {
  return items.filter((item) => item.group === group).sort((a, b) => a.order - b.order);
}
