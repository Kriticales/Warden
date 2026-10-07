/**
 * Montagem do CodeMirror 6 (SPEC T12): linguagem pelo nome do arquivo, o tema "Deep Dark" que
 * reproduz o `.editor` do design system (HANDOFF §3) e o painel de buscar e substituir em
 * português. Só o que o editor precisa: nada de autocompletar nem de dobrar trechos.
 */
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import { yaml } from '@codemirror/lang-yaml';
import {
  HighlightStyle,
  StreamLanguage,
  bracketMatching,
  syntaxHighlighting,
} from '@codemirror/language';
import { properties } from '@codemirror/legacy-modes/mode/properties';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { highlightSelectionMatches, search, searchKeymap } from '@codemirror/search';
import { EditorState, StateEffect, StateField, type Extension } from '@codemirror/state';
import {
  Decoration,
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  type DecorationSet,
} from '@codemirror/view';
import { tags } from '@lezer/highlight';

import { frasesDoEditor } from '../../../i18n/pt-BR/configs';
import type { EditorLanguage } from '../model';

function languageExtension(language: EditorLanguage): Extension {
  switch (language) {
    case 'toml':
      return StreamLanguage.define(toml);
    case 'properties':
      return StreamLanguage.define(properties);
    case 'json':
      return json();
    case 'yaml':
      return yaml();
    case 'javascript':
      return javascript();
    case 'typescript':
      return javascript({ typescript: true });
    case 'plain':
      return [];
  }
}

/** Cores do realce: as mesmas classes `tk-*` do protótipo, em variáveis do design system. */
const highlight = HighlightStyle.define([
  { tag: [tags.comment, tags.lineComment, tags.blockComment], color: 'var(--color-text-3)' },
  {
    tag: [tags.propertyName, tags.definition(tags.propertyName)],
    color: 'var(--color-primary-text)',
  },
  { tag: [tags.heading, tags.labelName, tags.namespace], color: 'var(--color-bone)' },
  { tag: [tags.string, tags.special(tags.string)], color: 'var(--color-ok-text)' },
  { tag: [tags.number, tags.integer, tags.float], color: 'var(--color-warn-text)' },
  { tag: [tags.bool, tags.null, tags.atom, tags.keyword], color: 'var(--color-ok-text)' },
  { tag: [tags.meta, tags.operator, tags.punctuation], color: 'var(--color-text-2)' },
]);

const theme = EditorView.theme({
  '&': {
    height: '100%',
    backgroundColor: 'var(--color-bg-sunken)',
    color: 'var(--color-text)',
    fontFamily: 'var(--font-mono)',
    fontSize: 'var(--text-xs)',
  },
  '&.cm-focused': {
    outline: 'var(--border-block) solid var(--color-focus)',
    outlineOffset: '-2px',
  },
  '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.75', overflow: 'auto' },
  '.cm-content': { caretColor: 'var(--color-text)', padding: 'var(--space-2) 0' },
  '.cm-cursor': { borderLeftColor: 'var(--color-text)' },
  '.cm-gutters': {
    backgroundColor: 'var(--color-bg-sunken)',
    color: 'var(--color-text-3)',
    border: 'none',
  },
  '.cm-lineNumbers .cm-gutterElement': {
    padding: '0 var(--space-3) 0 var(--space-2)',
    minWidth: '48px',
  },
  '.cm-activeLine': { backgroundColor: 'rgba(127, 243, 240, 0.05)' },
  '.cm-activeLineGutter': {
    backgroundColor: 'rgba(127, 243, 240, 0.05)',
    color: 'var(--color-text)',
  },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
    backgroundColor: 'var(--color-primary-soft)',
  },
  '.cm-selectionMatch': { backgroundColor: 'var(--color-surface-3)' },
  '.cm-line.cm-changed': {
    backgroundColor: 'var(--color-warn-soft)',
    boxShadow: 'inset 3px 0 0 var(--color-warn)',
  },
  '.cm-panels': {
    backgroundColor: 'var(--color-surface-1)',
    color: 'var(--color-text)',
    fontFamily: 'var(--font-ui)',
  },
  '.cm-panels-top': { borderBottom: 'var(--border-thin) solid var(--color-border)' },
  '.cm-search': {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: 'var(--space-2)',
    padding: 'var(--space-2) var(--space-3)',
  },
  '.cm-search br': { display: 'none' },
  '.cm-search input, .cm-search button': {
    font: 'inherit',
    fontSize: 'var(--text-sm)',
    padding: '2px var(--space-2)',
    color: 'var(--color-text)',
    backgroundColor: 'var(--color-bg-sunken)',
    border: 'var(--border-thin) solid var(--color-border-strong)',
  },
  '.cm-search button': { cursor: 'pointer', backgroundColor: 'var(--color-surface-2)' },
  '.cm-search label': { fontSize: 'var(--text-xs)', color: 'var(--color-text-2)' },
  '.cm-search [name=close]': { marginLeft: 'auto' },
  '.cm-searchMatch': {
    backgroundColor: 'var(--color-warn-soft)',
    outline: '1px solid var(--color-warn)',
  },
  '.cm-searchMatch-selected': { backgroundColor: 'var(--color-primary-soft)' },
});

/** Linhas diferentes do texto salvo, marcadas como no protótipo (`editor__line--changed`). */
export const setChangedLines = StateEffect.define<readonly number[]>();

const changedLines = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(setChangedLines)) {
        const doc = transaction.state.doc;
        const ranges = effect.value
          .filter((line) => line >= 1 && line <= doc.lines)
          .map((line) => Decoration.line({ class: 'cm-changed' }).range(doc.line(line).from));
        next = Decoration.set(ranges, true);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

export interface EditorOptions {
  doc: string;
  language: EditorLanguage;
  readOnly: boolean;
  /** Separador de linhas do arquivo: o texto gravado volta com os mesmos bytes. */
  lineSeparator: '\n' | '\r\n';
  ariaLabel: string;
  onChange: (text: string) => void;
  onCursor: (line: number, column: number) => void;
  onSave: () => void;
}

/** O estado inicial do editor (um novo por arquivo aberto). */
export function createEditorState(options: EditorOptions): EditorState {
  const { doc, language, readOnly, lineSeparator, ariaLabel, onChange, onCursor, onSave } = options;
  const extensions: Extension[] = [
    EditorState.lineSeparator.of(lineSeparator),
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    drawSelection(),
    bracketMatching(),
    highlightSelectionMatches(),
    history(),
    search({ top: true }),
    EditorState.phrases.of(frasesDoEditor),
    syntaxHighlighting(highlight),
    languageExtension(language),
    changedLines,
    theme,
    EditorView.contentAttributes.of({ 'aria-label': ariaLabel, spellcheck: 'false' }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChange(update.state.sliceDoc());
      }
      if (update.selectionSet || update.docChanged) {
        const head = update.state.selection.main.head;
        const line = update.state.doc.lineAt(head);
        onCursor(line.number, head - line.from + 1);
      }
    }),
    keymap.of([
      {
        key: 'Mod-s',
        preventDefault: true,
        run: () => {
          onSave();
          return true;
        },
      },
      ...searchKeymap,
      ...historyKeymap,
      ...defaultKeymap,
    ]),
  ];
  if (readOnly) {
    extensions.push(EditorState.readOnly.of(true), EditorView.editable.of(false));
  }
  return EditorState.create({ doc, extensions });
}
