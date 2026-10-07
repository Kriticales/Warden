/**
 * Botão "▶ Testar ▾" do cabeçalho do pack (SPEC T05 e T13; `testButton` em
 * `design/system/components.js`). Registrado em `pack-editor/header/slots.ts` (`test`).
 *
 * Estados: "Testar" (pronto); "Testando… ver progresso" com a barrinha de progresso (preparando);
 * "● Jogo aberto: ver teste" (jogo aberto). Nos dois últimos, o clique leva à tela do teste
 * (CA-T13-08). Com o jogo de outro pack aberto, o clique explica que é um jogo por vez e oferece
 * ir para o teste dele.
 *
 * O ▾ abre o menu em grupos (`menu-items.ts`), com os diálogos fora do menu (`testDialogs`).
 */
import * as MenuPrimitive from '@radix-ui/react-dropdown-menu';
import { useNavigate } from '@tanstack/react-router';
import { ChevronDown, Play } from 'lucide-react';
import { Fragment, useState, type CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../components/common/ConfirmDialog';
import { Loader } from '../../components/ui/loader';
import { buttonVariants } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { MenuContent, MenuItem, MenuSeparator } from '../../components/ui/menu';
import { showToast } from '../../components/ui/toast';
import { Tooltip } from '../../components/ui/tooltip';
import { cn } from '../../lib/cn';
import type { PackId } from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { useTestSettings } from '../pack-editor/api';
import type { HeaderSlotProps } from '../pack-editor/header/slots';
import { useRevealInstance, useTestSessions } from './api';
import {
  TEST_MENU_GROUPS,
  itemsOf,
  testDialogs,
  testMenuItems,
  type TestMenuContext,
} from './menu-items';
import { isActive, useGame, useTestRun, useTestStore } from './store';

export type TestButtonState = 'ready' | 'preparing' | 'running';

/** Abre a tela do teste do pack (de uma sessão gravada, com `sessionId`). */
export function useOpenTest() {
  const navigate = useNavigate();
  return (packId: PackId, sessionId?: string) => {
    void navigate({
      to: '/packs/$packId/teste',
      params: { packId },
      search: sessionId ? { sessao: sessionId } : {},
    });
  };
}

export function TestButton({ packId, pack }: HeaderSlotProps) {
  const { t } = useTranslation('teste');
  const game = useGame();
  const run = useTestRun(packId);
  const start = useTestStore((store) => store.start);
  const openTest = useOpenTest();
  const [menuOpen, setMenuOpen] = useState(false);
  const [dialog, setDialog] = useState<string | null>(null);
  const [otherOpen, setOtherOpen] = useState(false);

  const here = game?.packId === packId ? game : null;
  const other = game && game.packId !== packId ? game : null;
  const state: TestButtonState = here
    ? here.state === 'running'
      ? 'running'
      : 'preparing'
    : isActive(run)
      ? run?.status === 'running'
        ? 'running'
        : 'preparing'
      : 'ready';
  const progress = run?.progress;
  const percent = progress?.total ? Math.min(100, (progress.current / progress.total) * 100) : null;

  const onMain = () => {
    if (state !== 'ready') {
      openTest(packId);
      return;
    }
    if (other) {
      setOtherOpen(true);
      return;
    }
    void start(packId);
    openTest(packId);
  };

  const label = {
    ready: t('botao.testar'),
    preparing: t('botao.preparando'),
    running: t('botao.aberto'),
  }[state];

  return (
    <>
      <div className={cn('testbtn', `testbtn--${state}`)}>
        <span className="testbtn__antenna" aria-hidden="true">
          <i />
          <i />
          <i />
          <i />
        </span>
        <button
          type="button"
          className={cn(buttonVariants({ variant: 'primary' }), 'testbtn__main')}
          onClick={onMain}
        >
          {state === 'running' ? (
            <span className="live" aria-hidden="true" />
          ) : state === 'preparing' ? (
            <Loader />
          ) : (
            <Icon icon={Play} className="icon--fill" />
          )}
          <span>{label}</span>
          {state === 'preparing' ? (
            <span className="testbtn__progress" aria-hidden="true">
              <span style={{ '--p': `${String(percent ?? 12)}%` } as CSSProperties} />
            </span>
          ) : null}
        </button>
        <MenuPrimitive.Root modal={false} open={menuOpen} onOpenChange={setMenuOpen}>
          <Tooltip content={t('botao.maisOpcoes')}>
            <MenuPrimitive.Trigger
              className={cn(buttonVariants({ variant: 'primary' }), 'testbtn__more')}
            >
              <Icon icon={ChevronDown} />
              <span className="sr-only">{t('botao.maisOpcoes')}</span>
            </MenuPrimitive.Trigger>
          </Tooltip>
          {menuOpen ? (
            <TestMenu
              packId={packId}
              pack={pack}
              openTest={(sessionId) => {
                openTest(packId, sessionId);
              }}
              openDialog={setDialog}
            />
          ) : null}
        </MenuPrimitive.Root>
      </div>
      {testDialogs.map(({ id, component: Dialog }) => (
        <Dialog
          key={id}
          packId={packId}
          pack={pack}
          open={dialog === id}
          onOpenChange={(open) => {
            setDialog(open ? id : null);
          }}
        />
      ))}
      {other ? (
        <ConfirmDialog
          open={otherOpen}
          onOpenChange={setOtherOpen}
          destructive={false}
          title={t('outroJogo.titulo', { pack: other.packName })}
          description={t('outroJogo.texto', { pack: other.packName })}
          confirmLabel={t('outroJogo.irParaTeste')}
          cancelLabel={t('outroJogo.cancelar')}
          onConfirm={() => {
            openTest(other.packId);
          }}
        />
      ) : null}
    </>
  );
}

interface TestMenuProps extends Pick<HeaderSlotProps, 'packId' | 'pack'> {
  openTest: (sessionId?: string) => void;
  openDialog: (id: string) => void;
}

/** O conteúdo do ▾, montado só com o menu aberto (lê as sessões e a instância na hora). */
function TestMenu({ packId, pack, openTest, openDialog }: TestMenuProps) {
  const { t } = useTranslation('teste');
  const game = useGame();
  const sessions = useTestSessions(packId);
  const settings = useTestSettings(packId);
  const reveal = useRevealInstance(packId);
  const context: TestMenuContext = {
    packId,
    pack,
    game,
    lastSession: sessions.data?.[0] ?? null,
    instanceExists: settings.data?.instanceExists ?? false,
    openTest,
    openDialog,
    revealInstance: () => {
      reveal.mutate(undefined, {
        onError: (error) => {
          showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
        },
      });
    },
  };
  const groups = TEST_MENU_GROUPS.map((group) => ({
    ...group,
    items: itemsOf(group.id, testMenuItems),
  })).filter((group) => group.items.length > 0);

  return (
    <MenuContent>
      {groups.map((group, index) => (
        <Fragment key={group.id}>
          {index > 0 ? <MenuSeparator /> : null}
          {group.titleKey ? (
            <MenuPrimitive.Label className="menu__label">
              {t(group.titleKey as 'menu.perfil')}
            </MenuPrimitive.Label>
          ) : null}
          {group.items.map((item) => (
            <MenuItem
              key={item.id}
              icon={item.icon}
              description={item.description?.(context) ?? undefined}
              disabled={item.disabled?.(context) ?? false}
              onSelect={() => {
                item.onSelect(context);
              }}
            >
              {item.label(context)}
            </MenuItem>
          ))}
        </Fragment>
      ))}
    </MenuContent>
  );
}
