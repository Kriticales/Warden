/**
 * Modelo da seção Configs com as três origens (SPEC T12; ADR-0067; esboço aprovado
 * `design/bug-01/configs.html`, opção 1 com o "Comparar" da opção 3). Funções puras, sem IPC.
 *
 * - O seletor **Do pack | Do teste local | Comparar** ([`View`]) mora na URL (`origem`).
 * - A lista juntada (`config_overview`) cai em três grupos: **No pack**, **Só no teste** e
 *   **Só neste computador** ([`groupFiles`]).
 * - Os filtros "Mudou do padrão" e "Diferente do pack" são botões ([`filterFiles`]).
 */
import type { ConfigOrigin, PlacedConfig } from '../../../lib/ipc/bindings';

/** O que a seção mostra. */
export type View = 'pack' | 'test' | 'compare';

/** O valor de `origem` na URL. */
export type OriginParam = 'pack' | 'instancia' | 'comparar';

/**
 * A vista pela URL. Sem `origem`: com um arquivo pedido (Problemas e "Abrir a config" levam a
 * uma config do pack), o pack; sem arquivo, o teste local quando a instância existe (é lá que
 * os mods criam as configs); antes do primeiro teste, o pack.
 */
export function viewOf(
  param: OriginParam | undefined,
  testAvailable: boolean,
  withFile = false,
): View {
  if (param === 'pack') return 'pack';
  if (param === 'instancia') return 'test';
  if (param === 'comparar') return 'compare';
  return testAvailable && !withFile ? 'test' : 'pack';
}

export function paramOf(view: View): OriginParam {
  return view === 'pack' ? 'pack' : view === 'test' ? 'instancia' : 'comparar';
}

/** De onde o editor lê o arquivo na vista: o pack, ou o teste quando o arquivo existe lá. */
export function editorOrigin(view: View, file: PlacedConfig | undefined): ConfigOrigin {
  if (view === 'pack' || !file) return 'pack';
  return file.inTest ? 'instance' : 'pack';
}

/** Os três grupos da árvore. */
export interface Groups {
  /** No pack: vai para os jogadores. */
  pack: PlacedConfig[];
  /** Só no teste: o jogo criou e ninguém trouxe. */
  test: PlacedConfig[];
  /** Só neste computador: preferências pessoais. */
  personal: PlacedConfig[];
}

/**
 * Os grupos da vista. "Do pack" mostra só o que vai para os jogadores (o que está no pack e não
 * é pessoal); o teste e o Comparar mostram os três grupos, com as pessoais que existem no
 * teste (ou guardadas e ainda no pack, à espera da migração).
 */
export function groupFiles(files: readonly PlacedConfig[], view: View): Groups {
  const groups: Groups = { pack: [], test: [], personal: [] };
  for (const file of files) {
    if (file.place === 'pack') {
      groups.pack.push(file);
    } else if (view === 'pack') {
      continue;
    } else if (file.place === 'test') {
      groups.test.push(file);
    } else {
      groups.personal.push(file);
    }
  }
  return groups;
}

export interface Filters {
  /** Caminhos com alguma chave diferente do padrão ("Mudou do padrão"); `null` = desligado. */
  changed: ReadonlySet<string> | null;
  /** "Diferente do pack". */
  differs: boolean;
}

/** Os arquivos que passam nos filtros ligados. */
export function filterFiles(files: readonly PlacedConfig[], filters: Filters): PlacedConfig[] {
  return files.filter(
    (file) =>
      (filters.changed === null || filters.changed.has(file.path)) &&
      (!filters.differs || file.differs),
  );
}

/** Quantos arquivos o teste tem diferentes do pack. */
export function differingCount(files: readonly PlacedConfig[]): number {
  return files.filter((file) => file.place === 'pack' && file.differs).length;
}

/** Nomes amigáveis das preferências pessoais conhecidas (chave de `configs.pessoal.nomes`). */
const FRIENDLY: Record<string, 'opcoes' | 'optifine' | 'shaders' | 'minimapa' | 'mapa'> = {
  'options.txt': 'opcoes',
  'optionsof.txt': 'optifine',
  'optionsshaders.txt': 'shaders',
  'config/xaerominimap.txt': 'minimapa',
  'config/xaeroworldmap.txt': 'mapa',
};

export function friendlyKey(
  path: string,
): 'opcoes' | 'optifine' | 'shaders' | 'minimapa' | 'mapa' | null {
  return FRIENDLY[path.toLowerCase()] ?? null;
}

/** O arquivo da lista pelo caminho. */
export function byPath(
  files: readonly PlacedConfig[] | undefined,
  path: string | null,
): PlacedConfig | undefined {
  if (!files || path === null) return undefined;
  return files.find((file) => file.path === path);
}
