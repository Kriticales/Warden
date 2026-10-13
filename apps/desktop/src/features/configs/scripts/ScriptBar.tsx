/**
 * Peças da barra do editor de scripts (SPEC T26; protótipo `scripts`): a etiqueta com a versão
 * ("KubeJS 6 · Forge 1.20.1"), "Trechos prontos ▾", "Recarregar no jogo" (ou "Salvar e
 * recarregar", com alterações na instância de teste) e "Abrir no VS Code".
 *
 * Botão indisponível fica com `aria-disabled` (continua focável) e o motivo na dica e no nome
 * descrito, como pede a SPEC ("desabilitado com o motivo").
 */
import { snippet } from '@codemirror/autocomplete';
import { Braces, ChevronDown, ExternalLink, RefreshCw, Save } from 'lucide-react';
import { useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, type ButtonProps } from '../../../components/ui/button';
import { Menu, MenuContent, MenuItem, MenuTrigger } from '../../../components/ui/menu';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type { ConfigOrigin, PackId, ReloadRoute, Snippet } from '../../../lib/ipc/bindings';
import type { scripts as scriptTexts } from '../../../i18n/pt-BR/scripts';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import type { EditorAddonApi } from '../components/ConfigEditor';
import { useOpenVsCode } from './api';

/** Os trechos com nome no i18n (os `id` de `crates/warden-scripts/data/snippets/`). */
type SnippetId = keyof typeof scriptTexts.trechos;

/** A etiqueta com a ferramenta e o jogo. */
export function ScriptTag({ label }: { label: string }) {
  const { t } = useTranslation('scripts');
  return (
    <span className="tag tag--plain" title={t('barra.versao')}>
      <Braces className="icon icon--sm" aria-hidden="true" />
      {label}
    </span>
  );
}

/** Botão que, sem poder agir, continua focável e diz o motivo. */
function ReasonButton({
  reason,
  children,
  onClick,
  ...props
}: ButtonProps & { reason: string | null; children: ReactNode }) {
  const id = useId();
  if (reason === null) {
    return (
      <Button {...props} onClick={onClick}>
        {children}
      </Button>
    );
  }
  return (
    <Tooltip content={reason}>
      <span className="script-bar__reason">
        <Button {...props} aria-disabled="true" aria-describedby={id}>
          {children}
        </Button>
        <span id={id} className="sr-only">
          {reason}
        </span>
      </span>
    </Tooltip>
  );
}

export interface ScriptActionsProps {
  api: EditorAddonApi;
  packId: PackId;
  origin: ConfigOrigin;
  snippets: readonly Snippet[];
  route: ReloadRoute | undefined;
  instanceReady: boolean;
  reloading: boolean;
  onReload: () => Promise<void>;
}

/** Recarregar é possível (mesmo que só mostrando o comando). */
export function canReload(route: ReloadRoute | undefined): boolean {
  return route !== undefined && route.kind !== 'noGame' && route.kind !== 'notAScript';
}

export function ScriptActions({
  api,
  packId,
  origin,
  snippets,
  route,
  instanceReady,
  reloading,
  onReload,
}: ScriptActionsProps) {
  const { t } = useTranslation('scripts');
  const vscode = useOpenVsCode(packId);

  const insert = (body: string) => {
    const view = api.view();
    if (!view) return;
    const { from, to } = view.state.selection.main;
    snippet(body)(view, null, from, to);
    view.focus();
  };

  const saveAndReload = origin === 'instance' && api.dirty && canReload(route);

  return (
    <>
      {snippets.length > 0 ? (
        <Menu>
          <Tooltip content={t('barra.trechosDica')}>
            <MenuTrigger asChild>
              <Button size="sm" variant="ghost" iconEnd={ChevronDown}>
                {t('barra.trechos')}
              </Button>
            </MenuTrigger>
          </Tooltip>
          <MenuContent>
            {snippets.map((item) => (
              <MenuItem
                key={item.id}
                onSelect={() => {
                  insert(item.body);
                }}
              >
                {t(`trechos.${item.id as SnippetId}`)}
              </MenuItem>
            ))}
          </MenuContent>
        </Menu>
      ) : null}
      {route === undefined || route.kind === 'notAScript' ? null : saveAndReload ? (
        <Button
          size="sm"
          icon={Save}
          loading={reloading || api.saving}
          onClick={() => {
            void api.save().then((saved) => (saved ? onReload() : undefined));
          }}
        >
          {reloading ? t('barra.recarregando') : t('barra.salvarERecarregar')}
        </Button>
      ) : (
        <ReasonButton
          size="sm"
          icon={RefreshCw}
          loading={reloading}
          reason={route.kind === 'noGame' ? t('barra.semJogo') : null}
          onClick={() => {
            void onReload();
          }}
        >
          {reloading ? t('barra.recarregando') : t('barra.recarregar')}
        </ReasonButton>
      )}
      <ReasonButton
        size="sm"
        variant="ghost"
        icon={ExternalLink}
        loading={vscode.isPending}
        reason={instanceReady ? null : t('barra.vscodeSemInstancia')}
        onClick={() => {
          vscode.mutate(undefined, {
            onError: (error) => {
              showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
            },
          });
        }}
      >
        {t('barra.vscode')}
      </ReasonButton>
    </>
  );
}
