/**
 * O editor de texto (CodeMirror 6) de um arquivo. O CodeMirror guarda o documento; este
 * componente só o cria, o recria quando o arquivo (ou a leitura do disco) muda e avisa o que
 * o usuário digitou. `resetKey` identifica a leitura: mudou a chave, o texto volta a ser o
 * `initial`. `ref` expõe os comandos que a barra do editor chama.
 */
import { openSearchPanel } from '@codemirror/search';
import { EditorView } from '@codemirror/view';
import { useEffect, useImperativeHandle, useRef, type Ref } from 'react';

import type { EditorLanguage } from '../model';
import { createEditorState, setChangedLines } from './codemirror';
import { diffLines } from '../../../lib/diff';

export interface CodeEditorHandle {
  openSearch: () => void;
  focus: () => void;
  /** Troca todo o texto (Desfazer as alterações). */
  replaceAll: (text: string) => void;
  /** Põe o cursor no começo da linha (começa em 1), rola até ela e foca o editor. */
  revealLine: (line: number) => void;
}

/** Ir para uma linha (começa em 1). `request` muda a cada pedido, mesmo para a mesma linha. */
export interface LineRequest {
  line: number;
  request: number;
}

export interface CodeEditorProps {
  resetKey: string;
  initial: string;
  /** O texto salvo, para marcar as linhas alteradas. */
  saved: string;
  language: EditorLanguage;
  readOnly: boolean;
  lineSeparator: '\n' | '\r\n';
  ariaLabel: string;
  onChange: (text: string) => void;
  onCursor: (line: number, column: number) => void;
  /** Põe o cursor no começo da linha, centralizada, e o foco no editor. */
  goTo?: LineRequest | null;
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
  language,
  readOnly,
  lineSeparator,
  ariaLabel,
  onChange,
  onCursor,
  goTo = null,
  ref,
}: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  // Os callbacks mudam a cada render; o editor chama sempre a versão mais nova.
  const callbacks = useRef({ onChange, onCursor });
  const savedRef = useRef(saved);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    callbacks.current = { onChange, onCursor };
    savedRef.current = saved;
  });

  useEffect(() => {
    const parent = host.current;
    if (!parent) {
      return undefined;
    }
    const mark = () => {
      const current = view.current;
      if (current) {
        current.dispatch({
          effects: setChangedLines.of(
            changedLineNumbers(savedRef.current, current.state.sliceDoc()),
          ),
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
  const goToRequest = goTo?.request;
  const appliedRequest = useRef<number | null>(null);
  useEffect(() => {
    const current = view.current;
    if (!current || goToLine === undefined || goToRequest === appliedRequest.current) {
      return;
    }
    appliedRequest.current = goToRequest ?? null;
    const line = current.state.doc.line(Math.min(Math.max(goToLine, 1), current.state.doc.lines));
    current.dispatch({
      selection: { anchor: line.from },
      effects: EditorView.scrollIntoView(line.from, { y: 'center' }),
    });
    current.focus();
  }, [resetKey, goToLine, goToRequest]);

  // O texto salvo mudou (gravou): remarca as linhas.
  useEffect(() => {
    const current = view.current;
    if (current) {
      current.dispatch({
        effects: setChangedLines.of(changedLineNumbers(saved, current.state.sliceDoc())),
      });
    }
  }, [saved]);

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
    revealLine: (line) => {
      const current = view.current;
      if (current) {
        const doc = current.state.doc;
        const position = doc.line(Math.min(Math.max(line, 1), doc.lines)).from;
        current.dispatch({ selection: { anchor: position }, scrollIntoView: true });
        current.focus();
      }
    },
  }));

  return <div className="cfg-code" ref={host} />;
}
