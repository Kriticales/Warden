/**
 * Realce de ZenScript (CraftTweaker) para o CodeMirror 6, com `StreamLanguage` (ARCHITECTURE
 * §10.2). As palavras e as regras vêm da gramática TextMate do `Yesterday17/ZenScript` (MIT;
 * atribuição em `THIRD_PARTY.md`): pré-processador (`#loader`, `#priority`…), comentários `//`,
 * `#` e `/* … *\/`, strings com aspas simples e duplas, números, palavras-chave, tipos e os
 * colchetes angulares de IDs (`<item:minecraft:stone>`, `<minecraft:stone>` no 1.12.2).
 */
import { StreamLanguage, type StreamParser } from '@codemirror/language';

const PREPROCESSORS = new Set([
  'debug',
  'ignoreBracketErrors',
  'norun',
  'loader',
  'priority',
  'ikwid',
  'profile',
  'modloaded',
]);

const KEYWORDS = new Set([
  'import',
  'var',
  'val',
  'static',
  'global',
  'function',
  'return',
  'if',
  'else',
  'for',
  'while',
  'break',
  'continue',
  'in',
  'has',
  'as',
  'instanceof',
  'version',
  'void',
  'zenClass',
  'frigginClass',
  'zenConstructor',
  'frigginConstructor',
  'this',
  'class',
  'public',
  'private',
  'extends',
  'implements',
  'new',
  'enum',
  'expand',
]);

const TYPES = new Set(['any', 'bool', 'byte', 'short', 'int', 'long', 'float', 'double', 'string']);

const ATOMS = new Set(['true', 'false', 'null']);

interface State {
  /** Dentro de um comentário de bloco. */
  inBlockComment: boolean;
}

/** As regras do realce (exportadas para os testes). */
export const zenscriptParser: StreamParser<State> = {
  name: 'zenscript',
  startState: () => ({ inBlockComment: false }),
  token(stream, state) {
    if (state.inBlockComment) {
      if (stream.skipTo('*/')) {
        stream.match('*/');
        state.inBlockComment = false;
      } else {
        stream.skipToEnd();
      }
      return 'blockComment';
    }
    if (stream.eatSpace()) {
      return null;
    }
    if (stream.sol() && stream.match(/^#[A-Za-z]+/, false)) {
      const word = stream.match(/^#([A-Za-z]+)/);
      if (Array.isArray(word) && PREPROCESSORS.has(word[1] ?? '')) {
        return 'meta';
      }
      stream.skipToEnd();
      return 'lineComment';
    }
    if (stream.match('//') || stream.peek() === '#') {
      stream.skipToEnd();
      return 'lineComment';
    }
    if (stream.match('/*')) {
      state.inBlockComment = true;
      if (stream.skipTo('*/')) {
        stream.match('*/');
        state.inBlockComment = false;
      } else {
        stream.skipToEnd();
      }
      return 'blockComment';
    }
    // Colchete de ID: `<item:minecraft:stone>`, `<tag:items:forge:ingots/iron>`.
    if (stream.match(/^<[^<>\s]+:[^<>\s]+>/)) {
      return 'string.special';
    }
    const quote = stream.peek();
    if (quote === '"' || quote === "'") {
      stream.next();
      let escaped = false;
      for (;;) {
        const next = stream.next();
        if (next === undefined) break;
        if (next === quote && !escaped) break;
        escaped = !escaped && next === '\\';
      }
      return 'string';
    }
    if (stream.match(/^-?0[xX][0-9a-fA-F]+/) || stream.match(/^-?\d+\.\d+([eE][+-]?\d+)?[fFdD]?/)) {
      return 'number';
    }
    if (stream.match(/^-?\d+/)) {
      return 'number';
    }
    const word = stream.match(/^[A-Za-z_$][\w$]*/);
    if (Array.isArray(word)) {
      const text = word[0];
      if (KEYWORDS.has(text)) return 'keyword';
      if (TYPES.has(text)) return 'typeName';
      if (ATOMS.has(text)) return 'atom';
      return stream.match(/^\s*\(/, false) ? 'variableName.function' : 'variableName';
    }
    if (stream.match(/^(\.\.|[+\-*/%|&^~]=?|\|\||&&|[?:;$]|[<>]=?|==|=|!=|!)/)) {
      return 'operator';
    }
    stream.next();
    return 'punctuation';
  },
  languageData: {
    commentTokens: { line: '//', block: { open: '/*', close: '*/' } },
    closeBrackets: { brackets: ['(', '[', '{', "'", '"'] },
  },
};

/** A linguagem ZenScript do CodeMirror. */
export const zenscript = StreamLanguage.define(zenscriptParser);
