import { StringStream } from '@codemirror/language';
import { describe, expect, it } from 'vitest';

import { zenscriptParser } from './zenscript';

/** Os tokens de uma linha (texto e estilo). */
function tokens(
  line: string,
  state = zenscriptParser.startState?.(2) ?? { inBlockComment: false },
) {
  const stream = new StringStream(line, 4, 2);
  const out: [string, string | null][] = [];
  while (!stream.eol()) {
    const style = zenscriptParser.token(stream, state);
    out.push([stream.current(), style]);
    stream.start = stream.pos;
  }
  return { out, state };
}

describe('realce de ZenScript', () => {
  it('palavras, colchetes de ID, strings, números e comentários', () => {
    const { out } = tokens(
      'var x as int = 10; craftingTable.addShaped("nome", <item:minecraft:stone>, null); // fim',
    );
    const styled = out.filter(([, style]) => style !== null);
    expect(styled).toContainEqual(['var', 'keyword']);
    expect(styled).toContainEqual(['as', 'keyword']);
    expect(styled).toContainEqual(['int', 'typeName']);
    expect(styled).toContainEqual(['10', 'number']);
    expect(styled).toContainEqual(['"nome"', 'string']);
    expect(styled).toContainEqual(['<item:minecraft:stone>', 'string.special']);
    expect(styled).toContainEqual(['null', 'atom']);
    expect(styled).toContainEqual(['addShaped', 'variableName.function']);
    expect(styled).toContainEqual(['// fim', 'lineComment']);
  });

  it('pré-processador, comentário com # e bloco em várias linhas', () => {
    expect(tokens('#priority 10').out[0]).toEqual(['#priority', 'meta']);
    expect(tokens('# um comentário').out[0]).toEqual(['# um comentário', 'lineComment']);
    const first = tokens('/* começa');
    expect(first.out[0]).toEqual(['/* começa', 'blockComment']);
    expect(first.state.inBlockComment).toBe(true);
    const second = tokens('termina */ val', first.state);
    expect(second.out[0]).toEqual(['termina */', 'blockComment']);
    expect(second.out).toContainEqual(['val', 'keyword']);
  });

  it('1.12.2: colchete sem tipo e string com aspas escapadas', () => {
    const { out } = tokens("recipes.remove(<minecraft:stone:1>); 'a\\'b'");
    expect(out).toContainEqual(['<minecraft:stone:1>', 'string.special']);
    expect(out).toContainEqual(["'a\\'b'", 'string']);
  });
});
