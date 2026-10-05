/**
 * Avisos de terceiros (A-02): o JSON gerado por `cargo xtask notices` em `generated/avisos.json`
 * (fora do git; a CI gera antes de compilar o instalador).
 *
 * O arquivo é carregado sob demanda (`import.meta.glob`, num pedaço separado do bundle): sem ele,
 * o glob fica vazio e a compilação continua, e a tela explica que a lista não foi gerada.
 */

/** Versão do formato que esta tela entende (`SCHEMA` em `xtask/src/notices.rs`). */
export const NOTICES_SCHEMA = 1;

export type NoticeGroupId = 'rust' | 'npm' | 'packwiz' | 'fontes';

export interface NoticeItem {
  name: string;
  version: string;
  /** Expressão SPDX; vazia quando a licença não foi reconhecida (o texto continua lá). */
  license: string;
  url?: string;
  /** Índices em `Notices.texts`. */
  texts: number[];
}

export interface NoticeGroup {
  id: NoticeGroupId;
  items: NoticeItem[];
}

export interface Notices {
  schema: number;
  groups: NoticeGroup[];
  texts: string[];
}

const GROUP_IDS: readonly string[] = ['rust', 'npm', 'packwiz', 'fontes'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isItem(value: unknown, textCount: number): value is NoticeItem {
  return (
    isRecord(value) &&
    typeof value.name === 'string' &&
    typeof value.version === 'string' &&
    typeof value.license === 'string' &&
    (value.url === undefined || typeof value.url === 'string') &&
    Array.isArray(value.texts) &&
    value.texts.every(
      (index) =>
        typeof index === 'number' && Number.isInteger(index) && index >= 0 && index < textCount,
    )
  );
}

/** Confere o formato; devolve `null` se o arquivo não for do formato esperado. */
export function parseNotices(raw: unknown): Notices | null {
  if (!isRecord(raw) || raw.schema !== NOTICES_SCHEMA) return null;
  const { groups, texts } = raw;
  if (!Array.isArray(texts) || !texts.every((text) => typeof text === 'string')) return null;
  if (!Array.isArray(groups)) return null;
  const valid = groups.every(
    (group) =>
      isRecord(group) &&
      typeof group.id === 'string' &&
      GROUP_IDS.includes(group.id) &&
      Array.isArray(group.items) &&
      group.items.every((item) => isItem(item, texts.length)),
  );
  return valid ? (raw as unknown as Notices) : null;
}

type Loader = () => Promise<unknown>;

const GENERATED = import.meta.glob<unknown>('./generated/avisos.json', { import: 'default' });

/**
 * Lê os avisos gerados. `null`: o arquivo não existe nesta compilação ou não tem o formato
 * esperado (gerado por uma versão diferente do xtask).
 */
export async function loadNotices(
  modules: Record<string, Loader> = GENERATED,
): Promise<Notices | null> {
  const load = Object.values(modules)[0];
  if (!load) return null;
  return parseNotices(await load());
}

/** Quantos componentes há, somando os grupos. */
export function countItems(notices: Notices): number {
  return notices.groups.reduce((total, group) => total + group.items.length, 0);
}

/** Itens cujo nome ou licença contém o filtro (sem diferenciar maiúsculas). Grupos vazios saem. */
export function filterNotices(groups: NoticeGroup[], filter: string): NoticeGroup[] {
  const needle = filter.trim().toLocaleLowerCase('pt-BR');
  if (!needle) return groups;
  return groups
    .map((group) => ({
      ...group,
      items: group.items.filter(
        (item) =>
          item.name.toLocaleLowerCase('pt-BR').includes(needle) ||
          item.license.toLocaleLowerCase('pt-BR').includes(needle),
      ),
    }))
    .filter((group) => group.items.length > 0);
}

/** Se um componente está na lista (para os créditos que dependem do que foi compilado). */
export function hasItem(notices: Notices, group: NoticeGroupId, name: string): boolean {
  return notices.groups.some(
    (candidate) => candidate.id === group && candidate.items.some((item) => item.name === name),
  );
}
