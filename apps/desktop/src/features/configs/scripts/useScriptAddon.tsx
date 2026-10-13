/**
 * O editor de scripts dentro do editor de Configs (SPEC T26; ADR-0037): para um `.js` de
 * `kubejs/*_scripts/` ou um `.zs` de `scripts/`, num pack com KubeJS ou CraftTweaker, monta o
 * `EditorAddon` do `ConfigEditor` com o realce, o autocompletar de IDs, a barra, o aviso de
 * recarga e o painel "Erros dos scripts". Pack sem a ferramenta (ou arquivo que não é script):
 * `null`, e o arquivo abre como texto comum.
 *
 * Com o jogo aberto, salvar um script do pack o copia para a instância de teste (`onSaved`) e
 * oferece recarregar; veja `instanceSync.tsx`.
 */
import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { ConfigOrigin, PackId } from '../../../lib/ipc/bindings';
import type { AddonSaved, EditorAddon, EditorAddonApi } from '../components/ConfigEditor';
import { useGameKey, useReloadRoute, useScriptIds, useScriptProblems, useScriptsInfo } from './api';
import { scriptExtensions, setErrorLines, setIds } from './extensions';
import { InstanceSyncNote, useInstanceSync } from './instanceSync';
import {
  errorLines,
  flattenIds,
  formatLabel,
  problemsFor,
  scriptToolOf,
  snippetsFor,
  toolFor,
  toolLabel,
  toolName,
  type IdOption,
} from './model';
import { ReloadNoteView, useReloadAction } from './reload';
import { ScriptActions, ScriptTag } from './ScriptBar';
import { ScriptErrors } from './ScriptErrors';
import { LIVE_SOURCE } from './sources';
import './scripts.css';

export interface ScriptAddonParams {
  packId: PackId;
  origin: ConfigOrigin;
  path: string | null;
  loader: string | null;
  minecraft: string | null;
  /** Abre outro arquivo na mesma origem. */
  onOpenFile: (path: string) => void;
  /** Abre o mesmo arquivo na instância de teste. */
  onOpenInInstance: (path: string) => void;
}

interface FeedProps {
  api: EditorAddonApi;
  /** As extensões em uso: trocá-las recria os campos, então tudo é reenviado. */
  extensions: unknown;
  lines: readonly number[];
  options: readonly IdOption[];
}

/** Leva ao editor aberto os IDs do autocompletar e as linhas com erro. */
function EditorFeed({ api, extensions, lines, options }: FeedProps) {
  const key = lines.join(',');
  useEffect(() => {
    api.view()?.dispatch({ effects: setIds.of(options) });
  }, [api, extensions, options]);
  useEffect(() => {
    api.view()?.dispatch({ effects: setErrorLines.of(key ? key.split(',').map(Number) : []) });
  }, [api, extensions, key]);
  return null;
}

export function useScriptAddon({
  packId,
  origin,
  path,
  loader,
  minecraft,
  onOpenFile,
  onOpenInInstance,
}: ScriptAddonParams): EditorAddon | null {
  const { t } = useTranslation('scripts');
  const kind = path === null ? null : scriptToolOf(path);
  const info = useScriptsInfo(packId, kind !== null);
  const tool = kind && info.data ? toolFor(info.data.env, kind) : null;
  const active = kind !== null && tool !== null;
  const ids = useScriptIds(packId, active);
  const web = kind === 'kubejs' && (tool?.major ?? 0) >= 7;
  const problems = useScriptProblems(packId, active, web);
  const route = useReloadRoute(packId, active ? path : null);
  const game = useGameKey(packId);
  const reload = useReloadAction(packId);
  const sync = useInstanceSync(packId);
  const { copy } = sync;
  const running = game === 'running';
  // A gravação em curso veio de "Salvar e recarregar"; e se a instância ficou com o texto salvo.
  const reloadAfterSave = useRef(false);
  const synced = useRef(false);

  const onSaved = useCallback(
    async ({ previousHash }: AddonSaved) => {
      synced.current = false;
      if (origin !== 'pack' || !running || path === null) return;
      synced.current = await copy(path, previousHash, {
        overwrite: false,
        reload: reloadAfterSave.current,
      });
    },
    [origin, running, path, copy],
  );

  const options = useMemo(() => (ids.data ? flattenIds(ids.data) : []), [ids.data]);
  const extensions = useMemo(
    () =>
      kind && active
        ? scriptExtensions(kind, {
            kind: (value) => t(`ids.${value}`),
            source: (source) => (source === LIVE_SOURCE ? t('ids.jogoAberto') : source),
          })
        : null,
    [kind, active, t],
  );

  return useMemo(() => {
    if (!kind || !tool || !path || !info.data || !extensions) {
      return null;
    }
    const name = toolName(kind, tool);
    const list = problemsFor(problems.data?.problems ?? [], kind, path);
    const marks = errorLines(list, path);
    const run = reload.run;
    const saveAndReload = async (api: EditorAddonApi) => {
      reloadAfterSave.current = true;
      let saved: boolean;
      try {
        saved = await api.save();
      } finally {
        reloadAfterSave.current = false;
      }
      if (saved && (origin === 'instance' || synced.current)) {
        await run(path);
      }
    };
    return {
      onSaved,
      extensions,
      formatLabel: formatLabel(kind, tool),
      barStart: () => <ScriptTag label={toolLabel(kind, tool, loader, minecraft)} />,
      barEnd: (api) => (
        <ScriptActions
          api={api}
          packId={packId}
          snippets={snippetsFor(info.data, kind, path)}
          route={route.data}
          instanceReady={info.data.instanceReady}
          reloading={reload.pending}
          onReload={() => run(path)}
          onSaveAndReload={() => saveAndReload(api)}
        />
      ),
      notice: () => (
        <>
          {origin === 'pack' && running ? (
            <div className="script-note">
              <Alert
                kind="info"
                compact
                title={t('jogoAberto.titulo')}
                actions={
                  <Button
                    size="sm"
                    onClick={() => {
                      onOpenInInstance(path);
                    }}
                  >
                    {t('jogoAberto.acao')}
                  </Button>
                }
              >
                {t('jogoAberto.texto')}
              </Alert>
            </div>
          ) : null}
          <InstanceSyncNote
            note={sync.note}
            path={path}
            pending={sync.pending}
            reloading={reload.pending}
            onReload={() => {
              sync.closeNote();
              void run(path);
            }}
            onOverwrite={(previousHash, then) => {
              void copy(path, previousHash, { overwrite: true, reload: then }).then((ok) =>
                ok && then ? run(path) : undefined,
              );
            }}
            onOpenInInstance={() => {
              sync.closeNote();
              onOpenInInstance(path);
            }}
            onClose={sync.closeNote}
          />
          <ReloadNoteView note={reload.note} tool={name} onClose={reload.closeNote} />
        </>
      ),
      panel: (api) => (
        <>
          <EditorFeed api={api} extensions={extensions} lines={marks} options={options} />
          <ScriptErrors
            api={api}
            path={path}
            problems={list}
            data={problems.data}
            loading={problems.isPending}
            error={problems.isError ? problems.error : null}
            onRetry={() => {
              void problems.refetch();
            }}
            onOpenFile={onOpenFile}
          />
        </>
      ),
    } satisfies EditorAddon;
  }, [
    kind,
    tool,
    path,
    info.data,
    extensions,
    options,
    problems,
    route.data,
    reload,
    sync,
    copy,
    onSaved,
    running,
    origin,
    packId,
    loader,
    minecraft,
    onOpenFile,
    onOpenInInstance,
    t,
  ]);
}
