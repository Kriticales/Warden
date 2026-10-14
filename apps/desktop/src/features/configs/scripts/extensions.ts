/**
 * Extensões do CodeMirror do editor de scripts (SPEC T26): o realce de ZenScript, o
 * autocompletar de IDs (`@codemirror/autocomplete`), as linhas com erro do último teste e o
 * visual da lista de sugestões (`.complete` do design system).
 *
 * Os IDs chegam depois do editor aberto: entram no estado do editor pelo efeito `setIds`, a fonte
 * do autocompletar lê a lista do estado e filtrar é na memória (nada volta ao Rust a cada
 * tecla).
 */
import {
  autocompletion,
  completionKeymap,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete';
import { StateEffect, StateField, type Extension } from '@codemirror/state';
import { Decoration, EditorView, keymap, type DecorationSet } from '@codemirror/view';

import {
  idContext,
  insertText,
  matchIds,
  type IdKind,
  type IdOption,
  type ScriptToolKind,
} from './model';
import { zenscript } from './zenscript';

/** Os textos que a lista de sugestões mostra (do i18n de quem monta o editor). */
export interface CompletionTexts {
  kind: (kind: IdKind) => string;
  source: (source: string) => string;
}

/** Troca a lista de IDs do editor. */
export const setIds = StateEffect.define<readonly IdOption[]>();

const idsField = StateField.define<readonly IdOption[]>({
  create: () => [],
  update(value, transaction) {
    let next = value;
    for (const effect of transaction.effects) {
      if (effect.is(setIds)) next = effect.value;
    }
    return next;
  },
});

/** Fonte de sugestões de IDs para a ferramenta, com a lista do estado do editor. */
export function idCompletionSource(kind: ScriptToolKind, texts: CompletionTexts) {
  return (context: CompletionContext): CompletionResult | null => {
    const line = context.state.doc.lineAt(context.pos);
    const before = line.text.slice(0, context.pos - line.from);
    const found = idContext(before, kind);
    if (!found) return null;
    if (found.typed.replace(/^#/, '').length === 0 && !context.explicit) return null;
    const ids = context.state.field(idsField, false) ?? [];
    const options: Completion[] = matchIds(ids, found).map((option) => ({
      label: insertText(option, kind),
      detail: texts.kind(option.kind),
      info: texts.source(option.source),
      type: option.kind.endsWith('Tag') ? 'keyword' : 'constant',
    }));
    if (options.length === 0) return null;
    return { from: context.pos - found.length, options, filter: false };
  };
}

/** Marca as linhas com erro (`editor__line--error` do protótipo). */
export const setErrorLines = StateEffect.define<readonly number[]>();

const errorLinesField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(setErrorLines)) {
        const doc = transaction.state.doc;
        next = Decoration.set(
          effect.value
            .filter((line) => line >= 1 && line <= doc.lines)
            .map((line) =>
              Decoration.line({ class: 'cm-script-error' }).range(doc.line(line).from),
            ),
          true,
        );
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

const scriptTheme = EditorView.theme({
  '.cm-line.cm-script-error': {
    backgroundColor: 'var(--color-danger-soft)',
    boxShadow: 'inset 3px 0 0 var(--color-danger)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete': {
    backgroundColor: 'var(--color-bg-sunken)',
    border: 'var(--border-thin) solid var(--color-border-strong)',
    boxShadow: '0 0 0 1px var(--color-pixel-shadow), var(--shadow-block-2)',
    fontSize: 'var(--text-xs)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': {
    fontFamily: 'var(--font-mono)',
    maxHeight: '16em',
    minWidth: '340px',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
    padding: '3px var(--space-3)',
    color: 'var(--color-text-2)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--color-primary-soft)',
    boxShadow: 'inset var(--border-bar) 0 0 var(--color-primary)',
    color: 'var(--color-text)',
  },
  '.cm-completionMatchedText': {
    color: 'var(--color-primary-text)',
    fontWeight: 'var(--font-weight-semibold)',
    textDecoration: 'none',
  },
  '.cm-completionDetail': {
    marginLeft: 'var(--space-3)',
    color: 'var(--color-text-3)',
    fontStyle: 'normal',
    fontFamily: 'var(--font-ui)',
  },
  '.cm-completionInfo': {
    backgroundColor: 'var(--color-surface-1)',
    border: 'var(--border-thin) solid var(--color-border)',
    color: 'var(--color-text-2)',
    fontFamily: 'var(--font-ui)',
    padding: 'var(--space-1) var(--space-2)',
  },
  '.cm-completionIcon': { display: 'none' },
});

/** As extensões de um arquivo de script. */
export function scriptExtensions(kind: ScriptToolKind, texts: CompletionTexts): Extension {
  return [
    kind === 'crafttweaker' ? zenscript : [],
    idsField,
    autocompletion({
      override: [idCompletionSource(kind, texts)],
      icons: false,
      activateOnTyping: true,
    }),
    keymap.of(completionKeymap),
    errorLinesField,
    scriptTheme,
  ];
}
