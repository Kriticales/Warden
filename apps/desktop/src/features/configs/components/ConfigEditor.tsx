/**
 * O editor de um arquivo (SPEC T12; protótipo `.editor`): barra com o nome, a origem e as
 * ações; avisos contextuais; o CodeMirror; e o rodapé com o estado do arquivo.
 *
 * Salvar (CA-T12-03, CA-T12-05):
 * - mostra as diferenças antes de gravar, se a configuração `configDiffBeforeSave` estiver ligada;
 * - grava com o `hash` da leitura. Se o arquivo mudou fora do Warden, nada é gravado e a pessoa
 *   escolhe: Recarregar (descarta o que escreveu), Ver diferenças ou Sobrescrever.
 *
 * O texto vive no CodeMirror; aqui só se guarda a cópia para saber se há alterações. A leitura
 * em que o texto se baseia (`base`) só muda por escolha da pessoa ou depois de gravar.
 *
 * Formulário (C-04, `../form/`): o mesmo texto visto como campos. A edição pelo formulário troca
 * o texto daqui (com só o valor mudado), e salvar segue o mesmo caminho do texto.
 *
 * `addon` é o ponto de extensão do editor de scripts (C-07; SPEC T26): extensões do CodeMirror,
 * peças na barra, um aviso, um painel abaixo do texto e o nome do formato no rodapé.
 */
import type { Extension } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { Info, RotateCcw, Save, Search, TriangleAlert } from 'lucide-react';
import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type Ref,
} from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { DiffView } from '../../../components/common/DiffView';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import { formatBytes } from '../../../lib/format';
import type { ConfigContent, ConfigOrigin, PackId } from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { useConfigWrite } from '../api';
import {
  ConfigFormView,
  FallbackNotice,
  FormPending,
  ModeSwitch,
  TextOnlyNotice,
  useFormEditor,
} from '../form/ConfigForm';
import { CopyToDefaultconfigs, PreserveOption } from '../form/FileActions';
import { setEditorMode, useEditorMode } from '../form/model';
import { baseName, formatKey, languageOf, noticesFor } from '../model';
import {
  CodeEditor,
  changedLineNumbers,
  type CodeEditorHandle,
  type LineRequest,
} from './CodeEditor';

/** O que o editor oferece às peças do `addon`. */
export interface EditorAddonApi {
  dirty: boolean;
  saving: boolean;
  /** Grava sem perguntar nada. `true` se gravou (ou nada havia a gravar). */
  save: () => Promise<boolean>;
  gotoLine: (line: number, column?: number) => void;
  view: () => EditorView | null;
}

/** Ponto de extensão do editor (o editor de scripts, C-07). */
export interface EditorAddon {
  extensions: Extension;
  /** Depois da etiqueta da origem. */
  barStart?: (api: EditorAddonApi) => ReactNode;
  /** Antes de Buscar e Salvar. */
  barEnd?: (api: EditorAddonApi) => ReactNode;
  /** Abaixo dos avisos. */
  notice?: (api: EditorAddonApi) => ReactNode;
  /** Entre o texto e o rodapé. */
  panel?: (api: EditorAddonApi) => ReactNode;
  /** Formato no rodapé ("JavaScript · KubeJS 6"). */
  formatLabel?: string;
  /**
   * Depois de cada gravação bem-sucedida, com o `hash` do arquivo antes e depois. A gravação só
   * termina (e `save` só responde) quando a promessa termina: o editor de scripts copia aqui o
   * script salvo no pack para a instância de teste com o jogo aberto.
   */
  onSaved?: (saved: AddonSaved) => Promise<void>;
}

/** O que `onSaved` recebe. */
export interface AddonSaved {
  /** `hash` da leitura em que o texto se baseava (o arquivo antes de gravar). */
  previousHash: string;
  /** `hash` depois de gravar. */
  hash: string;
}

/** Uma peça do `addon` (desenhada como componente, para ler a API só no próprio render). */
function AddonSlot({
  render,
  api,
}: {
  render: ((api: EditorAddonApi) => ReactNode) | undefined;
  api: EditorAddonApi;
}) {
  return render ? <>{render(api)}</> : null;
}

/** O que a página usa para não perder alterações ao trocar de arquivo ou de seção. */
export interface EditorGuard {
  dirty: boolean;
  /** Grava sem perguntar nada. `true` se gravou (ou nada havia a gravar). */
  save: () => Promise<boolean>;
}

export interface ConfigEditorProps {
  packId: PackId;
  origin: ConfigOrigin;
  /** A leitura mais recente do disco (muda sozinha quando o arquivo muda por fora). */
  file: ConfigContent;
  loader: string | null;
  minecraft: string | null;
  /** Mostrar as diferenças antes de gravar (Configurações). */
  showDiff: boolean;
  onGuard: (guard: EditorGuard | null) => void;
  /** Linha para ir (resultado da busca em todas as configs, C-05). */
  goTo?: LineRequest | null;
  addon?: EditorAddon | null;
  ref?: Ref<CodeEditorHandle>;
}

type Dialogs = 'none' | 'save' | 'diff' | 'reload' | 'overwrite' | 'discard';

function isChangedOnDisk(error: unknown): boolean {
  const code = commandError(error)?.code;
  return code?.domain === 'project' && code.code === 'FILE_CHANGED_ON_DISK';
}

export function ConfigEditor({
  packId,
  origin,
  file,
  loader,
  minecraft,
  showDiff,
  onGuard,
  goTo = null,
  addon = null,
  ref,
}: ConfigEditorProps) {
  const { t } = useTranslation('configs');
  const write = useConfigWrite(packId);
  const editor = useRef<CodeEditorHandle>(null);
  useImperativeHandle(ref, () => ({
    openSearch: () => editor.current?.openSearch(),
    focus: () => editor.current?.focus(),
    replaceAll: (text) => editor.current?.replaceAll(text),
    gotoLine: (line, column) => editor.current?.gotoLine(line, column),
    view: () => editor.current?.view() ?? null,
  }));

  // O `addon` mais novo, sem refazer a gravação quando ele muda.
  const onSaved = useRef(addon?.onSaved);
  useEffect(() => {
    onSaved.current = addon?.onSaved;
  });

  const name = baseName(file.path);
  const readOnly = file.readOnly !== null;
  const [base, setBase] = useState({ text: file.text ?? '', hash: file.hash });
  const [text, setText] = useState(base.text);
  const [resetKey, setResetKey] = useState(0);
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const [dialog, setDialog] = useState<Dialogs>('none');
  const dirty = !readOnly && text !== base.text;
  const saving = write.isPending;
  // O arquivo no disco já não é aquele em que o texto se baseia.
  const external = !saving && file.hash !== base.hash;
  const conflict = external && dirty;
  const language = languageOf(file.path);
  const lineSeparator = file.lineEnding === 'crlf' ? '\r\n' : '\n';
  const [mode] = useEditorMode();
  const formState = useFormEditor({
    packId,
    path: file.path,
    text,
    editable: !readOnly && file.text !== null,
    active: mode === 'form',
  });
  // A edição pelo formulário não foi confirmada: este arquivo fica no texto, com o motivo.
  const [fallback, setFallback] = useState<unknown>(null);
  const showForm = mode === 'form' && formState.status === 'ready' && fallback === null;
  const formWaiting =
    mode === 'form' &&
    fallback === null &&
    (formState.status === 'loading' || formState.status === 'error');
  // "Editar este trecho como texto": a linha a mostrar quando o texto abrir. Vale até chegar
  // um pedido novo da página (`over` guarda qual era o dela); os números são negativos para não
  // coincidir com os da página.
  const [textLine, setTextLine] = useState<(LineRequest & { over: number | undefined }) | null>(
    null,
  );
  const nextLine: LineRequest | null =
    textLine !== null && textLine.over === goTo?.request ? textLine : goTo;
  // O pedido já atendido não vai para o editor recriado ao voltar do formulário para o texto.
  const [appliedLine, setAppliedLine] = useState<number | null>(null);
  const lineTarget = nextLine !== null && nextLine.request !== appliedLine ? nextLine : null;

  const reloadFrom = (content: ConfigContent) => {
    const next = { text: content.text ?? '', hash: content.hash };
    setBase(next);
    setText(next.text);
    setResetKey((key) => key + 1);
  };

  // Mudou por fora e não há nada a perder: acompanha o disco sem perguntar.
  if (external && !dirty) {
    reloadFrom(file);
  }

  const doWrite = useCallback(
    async (expectedHash: string, body: string): Promise<boolean> => {
      try {
        const saved = await write.mutateAsync({
          origin,
          path: file.path,
          text: body,
          expectedHash,
        });
        setBase({ text: body, hash: saved.hash });
        setDialog('none');
        showToast({
          kind: saved.changed ? 'ok' : 'info',
          title: t(saved.changed ? 'editor.salvoToast' : 'editor.semMudanca', { arquivo: name }),
        });
        await onSaved.current?.({ previousHash: expectedHash, hash: saved.hash });
        return true;
      } catch {
        setDialog('none');
        return false;
      }
    },
    [write, origin, file.path, name, t],
  );

  const requestSave = () => {
    if (!dirty || saving || conflict) {
      return;
    }
    if (showDiff) {
      setDialog('save');
    } else {
      void doWrite(base.hash, text);
    }
  };

  // Sempre a versão mais nova, sem refazer o registro a cada tecla.
  const latest = useRef({ dirty, text, hash: base.hash, doWrite });
  useEffect(() => {
    latest.current = { dirty, text, hash: base.hash, doWrite };
  });
  useEffect(() => {
    onGuard({
      dirty,
      save: async () => {
        const current = latest.current;
        return current.dirty ? current.doWrite(current.hash, current.text) : true;
      },
    });
  }, [dirty, onGuard]);
  useEffect(
    () => () => {
      onGuard(null);
    },
    [onGuard],
  );

  // Ctrl+S em qualquer ponto da tela, dentro do editor inclusive: o CodeMirror não tem atalho
  // próprio para salvar, e o evento sobe até aqui. Um só ouvinte = uma só gravação.
  const save = useRef(requestSave);
  useEffect(() => {
    save.current = requestSave;
  });
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        save.current();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  const addonSave = useCallback(async () => {
    const current = latest.current;
    return current.dirty ? current.doWrite(current.hash, current.text) : true;
  }, []);
  const addonGotoLine = useCallback((line: number, column?: number) => {
    editor.current?.gotoLine(line, column);
  }, []);
  const addonView = useCallback(() => editor.current?.view() ?? null, []);
  const api = useMemo<EditorAddonApi>(
    () => ({ dirty, saving, save: addonSave, gotoLine: addonGotoLine, view: addonView }),
    [dirty, saving, addonSave, addonGotoLine, addonView],
  );

  const changedLines = dirty ? changedLineNumbers(base.text, text).length : 0;
  const notices = noticesFor({ path: file.path, origin, loader, minecraft });
  const writeError =
    write.isError && !isChangedOnDisk(write.error) && commandError(write.error) !== null
      ? write.error
      : null;
  const format = formatKey(language);
  const lineEndingKey = file.lineEnding === 'mixed' ? 'mixed' : file.lineEnding;

  return (
    <section className="editor cfg-editor" aria-label={t('editor.area')}>
      <div className="editor__bar">
        <span className="editor__file">{file.path}</span>
        <span className={origin === 'instance' ? 'tag tag--warn' : 'tag tag--plain'}>
          {origin === 'instance' ? t('instancia.etiqueta') : t('pack.etiqueta')}
        </span>
        <AddonSlot render={addon?.barStart} api={api} />
        <span className="grow" />
        <AddonSlot render={addon?.barEnd} api={api} />
        {readOnly ? null : (
          <>
            <ModeSwitch
              mode={mode}
              state={formState}
              onChange={(next) => {
                setFallback(null);
                setEditorMode(next);
              }}
            />
            {showForm || formWaiting ? null : (
              <Button
                size="sm"
                variant="ghost"
                icon={Search}
                onClick={() => editor.current?.openSearch()}
              >
                {t('editor.buscar')}
              </Button>
            )}
            {dirty ? (
              <Button
                size="sm"
                variant="ghost"
                icon={RotateCcw}
                disabled={saving}
                onClick={() => {
                  setDialog('discard');
                }}
              >
                {t('editor.descartar')}
              </Button>
            ) : null}
            <Button
              size="sm"
              variant="primary"
              icon={Save}
              disabled={!dirty || conflict}
              loading={saving}
              onClick={requestSave}
            >
              {saving ? t('editor.salvando') : t('editor.salvar')}
            </Button>
          </>
        )}
      </div>

      {conflict ? (
        <div className="cfg-editor__notice">
          <Alert
            kind="danger"
            title={t('conflito.titulo')}
            actions={
              <>
                <Button
                  size="sm"
                  onClick={() => {
                    setDialog('reload');
                  }}
                >
                  {t('conflito.recarregar')}
                </Button>
                <Button
                  size="sm"
                  onClick={() => {
                    setDialog('diff');
                  }}
                >
                  {t('conflito.ver')}
                </Button>
                <Button
                  size="sm"
                  variant="danger-ghost"
                  onClick={() => {
                    setDialog('overwrite');
                  }}
                >
                  {t('conflito.sobrescrever')}
                </Button>
              </>
            }
          >
            {t('conflito.texto', { arquivo: file.path })}
          </Alert>
        </div>
      ) : null}
      {writeError ? (
        <div className="cfg-editor__notice">
          <ErrorPanel title={t('editor.erroGravar', { arquivo: name })} error={writeError} />
        </div>
      ) : null}
      {file.readOnly ? (
        <div className="cfg-editor__notice">
          <Alert kind="info" compact>
            {t(`somenteLeitura.${file.readOnly}`, { tamanho: formatBytes(file.size) })}
          </Alert>
        </div>
      ) : null}
      {notices.length > 0 ? (
        <ul className="cfg-editor__notices" aria-label={t('avisos.rotulo')}>
          {notices.map((notice) => (
            <li key={notice}>
              {notice === 'serverconfig' ? (
                <TriangleAlert className="icon icon--sm" aria-hidden="true" />
              ) : (
                <Info className="icon icon--sm" aria-hidden="true" />
              )}
              {t(`avisos.${notice}`)}
              {notice === 'serverconfig' ? (
                <CopyToDefaultconfigs packId={packId} path={file.path} dirty={dirty} />
              ) : notice === 'options' ? (
                <PreserveOption packId={packId} path={file.path} />
              ) : null}
            </li>
          ))}
        </ul>
      ) : null}
      <AddonSlot render={addon?.notice} api={api} />

      {mode === 'form' && fallback !== null ? (
        <div className="cfg-editor__notice">
          <FallbackNotice error={fallback} />
        </div>
      ) : mode === 'form' && formState.status === 'textOnly' && formState.reason ? (
        <div className="cfg-editor__notice">
          <TextOnlyNotice reason={formState.reason} />
        </div>
      ) : null}

      {showForm ? (
        <ConfigFormView
          form={formState.form}
          path={file.path}
          text={text}
          onText={setText}
          onOpenText={(line) => {
            setTextLine((previous) => ({
              line,
              request: Math.min(previous?.request ?? 0, 0) - 1,
              over: goTo?.request,
            }));
            setEditorMode('text');
          }}
          onFallback={setFallback}
        />
      ) : formWaiting ? (
        <FormPending state={formState} />
      ) : file.text === null ? (
        <p className="cfg-editor__empty t-sm t-2">
          {t(`somenteLeitura.${file.readOnly === 'tooLarge' ? 'tooLargeSemTexto' : 'binary'}`, {
            tamanho: formatBytes(file.size),
          })}
        </p>
      ) : (
        <CodeEditor
          ref={editor}
          resetKey={`${file.path}:${origin}:${String(resetKey)}`}
          initial={text}
          saved={base.text}
          language={language}
          readOnly={readOnly}
          lineSeparator={lineSeparator}
          ariaLabel={t('editor.rotuloTexto', { arquivo: file.path })}
          onChange={setText}
          onCursor={(line, column) => {
            setCursor({ line, column });
          }}
          goTo={lineTarget}
          onGoTo={setAppliedLine}
          {...(addon ? { extensions: addon.extensions } : {})}
        />
      )}
      <AddonSlot render={addon?.panel} api={api} />

      <div className="editor__foot">
        {dirty ? (
          <span className="t-warn" role="status">
            {changedLines > 0
              ? t('editor.naoSalvo', { count: changedLines })
              : t('editor.naoSalvoSemLinhas')}
          </span>
        ) : (
          <span>{t('editor.salvo')}</span>
        )}
        <span className="grow" />
        <span>
          {t('editor.rodape', {
            formato: addon?.formatLabel ?? t(`formato.${format}`),
            fimDeLinha: t(`fimDeLinha.${lineEndingKey}`),
            linha: cursor.line,
            coluna: cursor.column,
          })}
        </span>
      </div>

      <Dialog
        open={dialog === 'save'}
        onOpenChange={(open) => {
          if (!open) setDialog('none');
        }}
      >
        <DialogContent
          size="lg"
          title={t('salvarDialogo.titulo', { arquivo: name })}
          description={`${t('salvarDialogo.sub')} ${t('salvarDialogo.desligar')}`}
          footer={
            <>
              <DialogClose asChild>
                <Button variant="ghost">{t('salvarDialogo.continuar')}</Button>
              </DialogClose>
              <Button
                variant="primary"
                icon={Save}
                loading={saving}
                onClick={() => {
                  void doWrite(base.hash, text);
                }}
              >
                {t('salvarDialogo.confirmar')}
              </Button>
            </>
          }
        >
          <DiffView file={file.path} before={base.text} after={text} />
        </DialogContent>
      </Dialog>

      <Dialog
        open={dialog === 'diff'}
        onOpenChange={(open) => {
          if (!open) setDialog('none');
        }}
      >
        <DialogContent
          size="lg"
          title={t('conflito.verTitulo')}
          description={t('conflito.verSub')}
          footer={
            <>
              <DialogClose asChild>
                <Button variant="ghost">{t('conflito.voltar')}</Button>
              </DialogClose>
              <Button
                variant="danger"
                onClick={() => {
                  setDialog('overwrite');
                }}
              >
                {t('conflito.sobrescrever')}
              </Button>
            </>
          }
        >
          <DiffView file={file.path} before={file.text ?? ''} after={text} />
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={dialog === 'reload'}
        onOpenChange={(open) => {
          if (!open) setDialog('none');
        }}
        title={t('conflito.recarregarConfirmarTitulo', { arquivo: name })}
        description={t('conflito.recarregarConfirmarTexto')}
        confirmLabel={t('conflito.recarregarConfirmar')}
        onConfirm={() => {
          reloadFrom(file);
          setDialog('none');
        }}
      />
      <ConfirmDialog
        open={dialog === 'overwrite'}
        onOpenChange={(open) => {
          if (!open) setDialog('none');
        }}
        title={t('conflito.sobrescreverTitulo', { arquivo: name })}
        description={t('conflito.sobrescreverTexto')}
        confirmLabel={t('conflito.sobrescreverConfirmar')}
        confirmingLabel={t('editor.salvando')}
        onConfirm={async () => {
          await doWrite(file.hash, text);
        }}
      />
      <ConfirmDialog
        open={dialog === 'discard'}
        onOpenChange={(open) => {
          if (!open) setDialog('none');
        }}
        title={t('descartarDialogo.titulo', { arquivo: name })}
        description={t('descartarDialogo.texto')}
        confirmLabel={t('descartarDialogo.confirmar')}
        onConfirm={() => {
          if (editor.current) {
            editor.current.replaceAll(base.text);
          } else {
            setText(base.text);
          }
          setDialog('none');
        }}
      />
    </section>
  );
}
