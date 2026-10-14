/**
 * Ajuda comum dos E2E de desempenho (A-04; SPEC §8): o pack grande gerado, packs versionados
 * no git como o Warden os deixa, medidas feitas dentro do WebView e o resumo com mediana e
 * pior caso.
 *
 * O pack grande é o mesmo dos benchmarks das crates (`crates/warden-packwiz/benches/comum/`),
 * sem os jars (nenhuma tela medida aqui abre um jar): metafiles do Modrinth com IDs inventados,
 * que o servidor de fixtures responde (`modrinth.ts`), resource packs e shaders para chegar a 500
 * itens na lista de Mods, e as configs reais do corpus da `warden-configs` repetidas em lotes.
 *
 * Cada medida roda várias vezes e o teste confere a **mediana** contra a meta, com a folga do
 * build de debug ([`DEBUG_MARGIN`]): uma medida só varia demais entre máquinas da CI (ver a
 * CA-T02-01 do `packs.e2e.ts`). A mediana e o pior caso vão para o log, contra a meta exata,
 * para o `docs/DESEMPENHO.md`.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

import { MINECRAFT, perfProjectId, perfVersionId, type PerfFolder } from './modrinth';

const CORPUS = join(
  import.meta.dirname,
  '..',
  '..',
  '..',
  '..',
  'crates',
  'warden-configs',
  'tests',
  'corpus',
);

/** Pastas do corpus, na ordem do gerador das crates. */
const CORPUS_FOLDERS = ['toml', 'json', 'json5', 'properties', 'cfg', 'options'] as const;

export { MINECRAFT };
/** Versão do Fabric Loader do pack gerado. */
const FABRIC = '0.16.5';

export interface PackOptions {
  /** Nome do pack (`pack.toml`). */
  name: string;
  mods: number;
  resourcePacks: number;
  shaders: number;
  configs: number;
}

/** O pack das metas da SPEC §8 (300 mods, 500 configs) com 500 itens na lista (CA-T06-05). */
export const BIG_PACK: PackOptions = {
  name: 'Pack Grande',
  mods: 300,
  resourcePacks: 150,
  shaders: 50,
  configs: 500,
};

function sha256(data: string | Buffer): string {
  return createHash('sha256').update(data).digest('hex');
}

/** Os arquivos do corpus: pasta do formato, nome e bytes, em ordem. */
function corpus(): { folder: string; name: string; bytes: Buffer }[] {
  const files: { folder: string; name: string; bytes: Buffer }[] = [];
  for (const folder of CORPUS_FOLDERS) {
    const dir = join(CORPUS, folder);
    for (const name of readdirSync(dir).sort()) {
      if (name.startsWith('.') || name.endsWith('.md')) continue;
      const path = join(dir, name);
      if (!statSync(path).isFile()) continue;
      files.push({ folder, name, bytes: readFileSync(path) });
    }
  }
  return files;
}

function metafile(
  folder: PerfFolder,
  n: number,
  extension: string,
): { path: string; text: string } {
  const slug = `bench-${folder}-${String(n).padStart(3, '0')}`;
  const project = perfProjectId(folder, n);
  const version = perfVersionId(folder, n);
  const file = `${slug}-1.0.${String(n)}.${extension}`;
  const side = n % 10 === 0 ? 'client' : n % 10 === 5 ? 'server' : 'both';
  const text = [
    `name = "Item de Bancada ${folder} ${String(n).padStart(3, '0')}"`,
    `filename = "${file}"`,
    `side = "${folder === 'mods' ? side : 'client'}"`,
    '',
    '[download]',
    `url = "https://cdn.modrinth.com/data/${project}/versions/${version}/${file}"`,
    'hash-format = "sha512"',
    `hash = "${createHash('sha512').update(file).digest('hex')}"`,
    '',
    '[update]',
    '[update.modrinth]',
    `mod-id = "${project}"`,
    `version = "${version}"`,
    '',
  ].join('\n');
  return { path: `${folder}/${slug}.pw.toml`, text };
}

/** Grava o pack em `dir` (que pode já existir) e devolve os caminhos das configs. */
export function writePack(dir: string, options: PackOptions): string[] {
  const index: { file: string; hash: string; metafile: boolean }[] = [];
  const write = (path: string, data: string | Buffer, isMetafile: boolean) => {
    const full = join(dir, path);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, data);
    index.push({ file: path, hash: sha256(data), metafile: isMetafile });
  };
  const groups = [
    ['mods', options.mods, 'jar'],
    ['resourcepacks', options.resourcePacks, 'zip'],
    ['shaderpacks', options.shaders, 'zip'],
  ] as const;
  for (const [folder, count, extension] of groups) {
    for (let n = 0; n < count; n += 1) {
      const { path, text } = metafile(folder, n, extension);
      write(path, text, true);
    }
  }
  const samples = corpus();
  const configs: string[] = [];
  for (let position = 0; position < options.configs; position += 1) {
    const sample = samples[position % samples.length];
    if (!sample) break;
    const batch = Math.floor(position / samples.length);
    const path =
      sample.folder === 'options'
        ? `config/lote${String(batch)}/${sample.name.replace(/\.txt$/, '')}/options.txt`
        : `config/lote${String(batch)}/${sample.name}`;
    write(path, sample.bytes, false);
    configs.push(path);
  }
  index.sort((a, b) => (a.file < b.file ? -1 : a.file > b.file ? 1 : 0));
  const indexText = [
    'hash-format = "sha256"',
    ...index.flatMap((entry) => [
      '',
      '[[files]]',
      `file = ${JSON.stringify(entry.file)}`,
      `hash = "${entry.hash}"`,
      ...(entry.metafile ? ['metafile = true'] : []),
    ]),
    '',
  ].join('\n');
  writeFileSync(join(dir, 'index.toml'), indexText);
  writeFileSync(
    join(dir, 'pack.toml'),
    [
      `name = ${JSON.stringify(options.name)}`,
      'author = "Warden"',
      'version = "1.0.0"',
      'pack-format = "packwiz:1.1.0"',
      '',
      '[index]',
      'file = "index.toml"',
      'hash-format = "sha256"',
      `hash = "${sha256(indexText)}"`,
      '',
      '[versions]',
      `minecraft = "${MINECRAFT}"`,
      `fabric = "${FABRIC}"`,
      '',
    ].join('\n'),
  );
  return configs;
}

function git(dir: string, args: string[]): void {
  execFileSync(
    'git',
    [
      '-c',
      'user.name=Warden',
      '-c',
      'user.email=warden@localhost',
      '-c',
      'core.autocrlf=false',
      '-c',
      'commit.gpgsign=false',
      '-c',
      'tag.gpgsign=false',
      ...args,
    ],
    { cwd: dir, stdio: 'pipe' },
  );
}

/**
 * Versiona o pack como o Warden deixa: o ponto inicial no git e, com `saved`, a versão 1.0.0
 * (tag `v1.0.0` anotada) seguida de duas alterações ainda não salvas.
 */
export function versionPack(dir: string, saved: boolean): void {
  writeFileSync(join(dir, '.gitattributes'), '* -text\n');
  writeFileSync(join(dir, '.gitignore'), '.warden/cache/\n');
  git(dir, ['init', '-q', '-b', 'main']);
  git(dir, ['add', '-A']);
  git(dir, ['commit', '-q', '-m', 'Pack criado']);
  if (!saved) return;
  writeFileSync(join(dir, 'config', 'salva.toml'), 'valor = 1\n');
  git(dir, ['add', '-A']);
  git(dir, ['commit', '-q', '-m', 'Versão 1.0.0']);
  git(dir, ['tag', '-a', 'v1.0.0', '-m', 'Primeira versão']);
  writeFileSync(join(dir, 'config', 'alterada.toml'), 'valor = 2\n');
  writeFileSync(join(dir, 'config', 'salva.toml'), 'valor = 3\n');
}

const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

/** Um ULID válido e único por número (identificador de pack no `packs.json`). */
export function ulid(n: number): string {
  let tail = '';
  let rest = n;
  for (let index = 0; index < 8; index += 1) {
    tail = (CROCKFORD[rest % 32] ?? '0') + tail;
    rest = Math.floor(rest / 32);
  }
  return `01JC${'0'.repeat(14)}${tail}`;
}

/** Mediana e pior caso de uma série de medidas (ms). */
export function summary(values: readonly number[]): { median: number; worst: number } {
  const sorted = [...values].sort((a, b) => a - b);
  return {
    median: sorted[Math.floor(sorted.length / 2)] ?? Number.NaN,
    worst: sorted[sorted.length - 1] ?? Number.NaN,
  };
}

/**
 * Folga do E2E sobre as metas da SPEC §8. As metas valem para o app instalado (build de release)
 * na máquina de referência; os E2E rodam o build de debug nas máquinas da CI, sem GPU e com
 * tempos que variam muito de uma rodada para outra (no Windows, a mesma exportação levou 4 s
 * numa rodada e 6,7 s na seguinte). O teste confere a mediana contra a meta com esta folga; a
 * linha do log compara com a meta exata, e os benchmarks (release) conferem a meta exata.
 */
export const DEBUG_MARGIN = 1.5;

/** O limite que o E2E confere para uma meta (ms). */
export function e2eLimit(goalMs: number, margin = DEBUG_MARGIN): number {
  return goalMs * margin;
}

/**
 * Imprime a linha do resumo (`A-04 E2E | medida | mediana | pior | meta | situação`, com a
 * situação contra a meta exata) e devolve a mediana, que o teste confere contra
 * [`e2eLimit`].
 */
export function report(measure: string, values: readonly number[], goalMs: number): number {
  const { median, worst } = summary(values);
  const state = median < goalMs ? 'ok' : 'acima da meta';
  process.stdout.write(
    `A-04 E2E | ${measure} | mediana ${median.toFixed(0)} ms | pior ${worst.toFixed(0)} ms | ` +
      `meta < ${String(goalMs)} ms | ${state} | medidas: ${values.map((v) => v.toFixed(0)).join(', ')}\n`,
  );
  return median;
}
