/**
 * O editor de texto (CodeMirror 6) de um arquivo. O CodeMirror guarda o documento; este
 * componente só o cria, o recria quando o arquivo (ou a leitura do disco) muda e avisa o que
 * o usuário digitou. `resetKey` identifica a leitura: mudou a chave, o texto volta a ser o
 * `initial`. `ref` expõe os comandos que a barra do editor chama.
 *
 * `extensions` (o editor de scripts, C-07) fica num `Compartment`: trocar a lista reconfigura
 * o editor sem recriá-lo nem perder o que foi digitado.
 */
import { openSearchPanel } from '@codemirror/search';
import { Compartment, type Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { useEffect, useImperativeHandle, useRef, type Ref } from 'react';

import type { EditorLanguage } from '../model';
import { createEditorState, setChangedLines, setPackLines } from './codemirror';
import { diffLines } from '../../../lib/diff';

export interface CodeEditorHandle {
  openSearch: () => void;
  focus: () => void;
  /** Troca todo o texto (Desfazer as alterações). */
  replaceAll: (text: string) => void;
  /** O `EditorView` aberto (trechos prontos do editor de scripts). */
  view: () => EditorView | null;
}

/**
 * Ir para uma linha (começa em 1). `request` muda a cada pedido, mesmo para a mesma linha. É o
 * único "ir para a linha" do editor: vale também para o editor que acabou de ser criado (arquivo
 * novo, ou o texto aberto a partir do formulário).
 */
export interface LineRequest {
  line: number;
  /** Coluna (começa em 1); sem ela, o começo da linha (o erro de um script traz a coluna). */
  column?: number;
  request: number;
}

export interface CodeEditorProps {
  resetKey: string;
  initial: string;
  /** O texto salvo, para marcar as linhas alteradas. */
  saved: string;
  /** O texto do pack, para marcar as linhas diferentes dele (o teste local aberto). */
  compareTo?: string | null;
  language: EditorLanguage;
  readOnly: boolean;
  lineSeparator: '\n' | '\r\n';
  ariaLabel: string;
  onChange: (text: string) => void;
  onCursor: (line: number, column: number) => void;
  /**
   * Põe o cursor na linha (no começo, ou na coluna pedida), centralizada, e o foco no editor;
   * uma vez por pedido.
   */
  goTo?: LineRequest | null;
  /**
   * Avisa que o pedido foi atendido. Quem recria o editor (formulário ↔ texto) para de mandar o
   * pedido atendido, e o editor novo não volta para a linha antiga.
   */
  onGoTo?: (request: number) => void;
  extensions?: Extension;
  ref?: Ref<CodeEditorHandle>;
}

/** Números (1..) das linhas do texto novo que não existem no salvo. */
export function changedLineNumbers(saved: string, current: string): number[] {
  if (saved === current) {
    return [];
  }
  return diffLines(saved, current, { context: null }).flatMap((line) =>
    line.kind === 'add' ? [line.newNo] : [],
  );
}

const MARK_DELAY_MS = 200;

export function CodeEditor({
  resetKey,
  initial,
  saved,
  compareTo = null,
  language,
  readOnly,
  lineSeparator,
  ariaLabel,
  onChange,
  onCursor,
  goTo = null,
  onGoTo,
  extensions,
  ref,
}: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const extra = useRef(new Compartment());
  const extensionsRef = useRef<Extension>(extensions ?? []);
  // Os callbacks mudam a cada render; o editor chama sempre a versão mais nova.
  const callbacks = useRef({ onChange, onCursor, onGoTo });
  const savedRef = useRef(saved);
  const compareRef = useRef(compareTo);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    callbacks.current = { onChange, onCursor, onGoTo };
    savedRef.current = saved;
    compareRef.current = compareTo;
  });

  useEffect(() => {
    const parent = host.current;
    if (!parent) {
      return undefined;
    }
    const mark = () => {
      const current = view.current;
      if (current) {
        const text = current.state.sliceDoc();
        current.dispatch({
          effects: [
            setChangedLines.of(changedLineNumbers(savedRef.current, text)),
            setPackLines.of(
              compareRef.current === null ? [] : changedLineNumbers(compareRef.current, text),
            ),
          ],
        });
      }
    };
    const editor = new EditorView({
      parent,
      state: createEditorState({
        doc: initial,
        language,
        readOnly,
        lineSeparator,
        ariaLabel,
        onChange: (text) => {
          callbacks.current.onChange(text);
          clearTimeout(timer.current);
          timer.current = setTimeout(mark, MARK_DELAY_MS);
        },
        onCursor: (line, column) => {
          callbacks.current.onCursor(line, column);
        },
        extra: extra.current.of(extensionsRef.current),
      }),
    });
    view.current = editor;
    mark();
    return () => {
      clearTimeout(timer.current);
      editor.destroy();
      view.current = null;
    };
    // Recriar só quando o arquivo muda: o texto digitado vive no CodeMirror, não nas props.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resetKey, language, readOnly, lineSeparator, ariaLabel]);

  // Ir para a linha pedida (também logo depois de criar o editor de um arquivo novo). Cada
  // pedido vale uma vez: recarregar o arquivo (edição por fora) não puxa o cursor nem o foco.
  const goToLine = goTo?.line;
  const goToColumn = goTo?.column ?? 1;
  const goToRequest = goTo?.request;
  const appliedRequest = useRef<number | null>(null);
  useEffect(() => {
    const current = view.current;
    if (!current || goToLine === undefined || goToRequest === appliedRequest.current) {
      return;
    }
    appliedRequest.current = goToRequest ?? null;
    const line = current.state.doc.line(Math.min(Math.max(goToLine, 1), current.state.doc.lines));
    const position = Math.min(line.from + Math.max(goToColumn - 1, 0), line.to);
    current.dispatch({
      selection: { anchor: position },
      effects: EditorView.scrollIntoView(position, { y: 'center' }),
    });
    current.focus();
    if (goToRequest !== undefined) {
      callbacks.current.onGoTo?.(goToRequest);
    }
  }, [resetKey, goToLine, goToColumn, goToRequest]);

  // Extensões novas: reconfigura sem recriar.
  useEffect(() => {
    extensionsRef.current = extensions ?? [];
    view.current?.dispatch({ effects: extra.current.reconfigure(extensionsRef.current) });
  }, [extensions]);

  // O texto salvo (gravou) ou o do pack mudou: remarca as linhas.
  useEffect(() => {
    const current = view.current;
    if (current) {
      const text = current.state.sliceDoc();
      current.dispatch({
        effects: [
          setChangedLines.of(changedLineNumbers(saved, text)),
          setPackLines.of(compareTo === null ? [] : changedLineNumbers(compareTo, text)),
        ],
      });
    }
  }, [saved, compareTo]);

  useImperativeHandle(ref, () => ({
    openSearch: () => {
      if (view.current) {
        openSearchPanel(view.current);
      }
    },
    focus: () => {
      view.current?.focus();
    },
    replaceAll: (text) => {
      const current = view.current;
      if (current) {
        current.dispatch({ changes: { from: 0, to: current.state.doc.length, insert: text } });
      }
    },
    view: () => view.current,
  }));

  return <div className="cfg-code" ref={host} />;
}
