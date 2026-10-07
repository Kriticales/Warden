import { describe, expect, it } from 'vitest';

import type { ConsoleLine } from '../../../lib/ipc/bindings';
import {
  filterLines,
  highlightRanges,
  lineTone,
  matchesLevel,
  normalizeForSearch,
  type ConsoleFilter,
} from './filter';

let seq = 0;
function line(overrides: Partial<ConsoleLine> = {}): ConsoleLine {
  seq += 1;
  return {
    seq,
    atMs: null,
    time: '19:53:26',
    level: 'info',
    logger: 'minecraft/Minecraft',
    thread: 'Render thread',
    text: `linha ${String(seq)}`,
    params: {},
    origin: 'game',
    stream: 'stdout',
    ...overrides,
  };
}

const searchable = (l: ConsoleLine) => normalizeForSearch(`${l.text}\n${l.logger ?? ''}`);
const run = (lines: ConsoleLine[], filter: Partial<ConsoleFilter>) =>
  filterLines(lines, { level: 'all', query: '', ...filter }, searchable).map((r) => r.line.text);

describe('matchesLevel', () => {
  it('"Todas as linhas" aceita tudo, até sem nível', () => {
    expect(matchesLevel(null, 'all')).toBe(true);
    expect(matchesLevel('trace', 'all')).toBe(true);
  });

  it('"Só avisos e erros" aceita aviso, erro e fatal', () => {
    expect(matchesLevel('warn', 'warn')).toBe(true);
    expect(matchesLevel('error', 'warn')).toBe(true);
    expect(matchesLevel('fatal', 'warn')).toBe(true);
    expect(matchesLevel('info', 'warn')).toBe(false);
    expect(matchesLevel('debug', 'warn')).toBe(false);
    expect(matchesLevel(null, 'warn')).toBe(false);
  });

  it('"Só erros" aceita erro e fatal', () => {
    expect(matchesLevel('error', 'error')).toBe(true);
    expect(matchesLevel('fatal', 'error')).toBe(true);
    expect(matchesLevel('warn', 'error')).toBe(false);
  });
});

describe('lineTone', () => {
  it('Warden pela origem; erro e fatal; aviso; debug e trace; o resto, info', () => {
    expect(lineTone(line({ origin: 'warden', level: 'error' }), 'error')).toBe('warden');
    expect(lineTone(line(), 'fatal')).toBe('error');
    expect(lineTone(line(), 'error')).toBe('error');
    expect(lineTone(line(), 'warn')).toBe('warn');
    expect(lineTone(line(), 'trace')).toBe('debug');
    expect(lineTone(line(), 'debug')).toBe('debug');
    expect(lineTone(line(), 'info')).toBe('info');
    expect(lineTone(line(), null)).toBe('info');
  });
});

describe('filterLines', () => {
  const lines = [
    line({ text: 'Carregando texturas', level: 'info' }),
    line({ text: 'Mod sem versão', level: 'warn', logger: 'fml/Loader' }),
    line({ text: 'Falha ao ler config', level: 'error' }),
    line({ text: '\tat net.minecraft.Foo.bar(Foo.java:10)', level: null }),
    line({ text: 'Detalhe', level: 'debug' }),
  ];

  it('sem filtro, devolve tudo na ordem', () => {
    expect(run(lines, {})).toHaveLength(5);
  });

  it('por nível; a linha sem nível segue a última linha com nível (stack trace)', () => {
    expect(run(lines, { level: 'warn' })).toEqual([
      'Mod sem versão',
      'Falha ao ler config',
      '\tat net.minecraft.Foo.bar(Foo.java:10)',
    ]);
    expect(run(lines, { level: 'error' })).toEqual([
      'Falha ao ler config',
      '\tat net.minecraft.Foo.bar(Foo.java:10)',
    ]);
  });

  it('o nível herdado vale também para a cor', () => {
    const rows = filterLines(lines, { level: 'all', query: '' }, searchable);
    expect(rows[3]?.level).toBe('error');
    expect(rows[0]?.level).toBe('info');
  });

  it('busca sem diferenciar maiúsculas e acentos, no texto e na origem', () => {
    expect(run(lines, { query: 'VERSAO' })).toEqual(['Mod sem versão']);
    expect(run(lines, { query: 'fml/' })).toEqual(['Mod sem versão']);
    expect(run(lines, { query: '  config ' })).toEqual(['Falha ao ler config']);
    expect(run(lines, { query: 'nada disso' })).toEqual([]);
  });

  it('nível e busca juntos', () => {
    expect(run(lines, { level: 'error', query: 'foo.java' })).toEqual([
      '\tat net.minecraft.Foo.bar(Foo.java:10)',
    ]);
    expect(run(lines, { level: 'error', query: 'texturas' })).toEqual([]);
  });
});

describe('highlightRanges', () => {
  it('acha todas as ocorrências, sem maiúsculas e acentos, em posições do texto original', () => {
    const text = 'Gráficos: GRAFICOS e graficos';
    const ranges = highlightRanges(text, 'graficos');
    expect(ranges.map(([a, b]) => text.slice(a, b))).toEqual(['Gráficos', 'GRAFICOS', 'graficos']);
  });

  it('com caractere decomposto (acento separado), o trecho inclui o acento', () => {
    const text = 'versão nova';
    const [range] = highlightRanges(text, 'versão');
    expect(range && text.slice(range[0], range[1])).toBe('versão');
  });

  it('busca vazia não destaca nada', () => {
    expect(highlightRanges('qualquer', '   ')).toEqual([]);
  });
});
