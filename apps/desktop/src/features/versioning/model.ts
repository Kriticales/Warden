/**
 * Regras puras de Salvar versão e do Histórico (SPEC T16 e T17): estado de cada versão,
 * agrupamento das mudanças, resumo do changelog e a data como a tela mostra.
 *
 * Nada aqui fala com o backend nem com o React, para ser testado em tabela.
 */
import type { ChangeSet, ItemChange, SavedVersion } from '../../lib/ipc/bindings';
import { dayRelation, formatDate, formatTime, type DayRelation } from '../../lib/format';

/** Estado de uma versão salva (SPEC T17). */
export type VersionState = 'saved' | 'final' | 'published';

export function versionState(version: Pick<SavedVersion, 'isFinal' | 'isPublished'>): VersionState {
  if (version.isPublished) {
    return 'published';
  }
  return version.isFinal ? 'final' : 'saved';
}

/** As mudanças de um {@link ChangeSet}, nos mesmos grupos do changelog (`CHANGELOG.md`). */
export interface ChangeGroups {
  added: ItemChange[];
  removed: ItemChange[];
  updated: ItemChange[];
  adjusted: ItemChange[];
  /** Resource packs e shaders, de qualquer tipo de mudança. */
  others: ItemChange[];
  configs: string[];
  /** Há mudança de Minecraft ou de loader. */
  platform: boolean;
}

export function groupChanges(changes: ChangeSet): ChangeGroups {
  const mods = (kind: ItemChange['kind']) =>
    changes.items.filter((item) => item.category === 'mod' && item.kind === kind);
  return {
    added: mods('added'),
    removed: mods('removed'),
    updated: mods('updated'),
    adjusted: mods('adjusted'),
    others: changes.items.filter((item) => item.category !== 'mod'),
    configs: changes.configs,
    platform: changes.minecraft !== null || changes.loaders.length > 0,
  };
}

/** Não há item, config nem Minecraft/loader mudado (só arquivos de controle). */
export function hasOnlyControlFiles(changes: ChangeSet): boolean {
  const groups = groupChanges(changes);
  return (
    changes.files.length > 0 &&
    !groups.platform &&
    groups.configs.length === 0 &&
    changes.items.length === 0
  );
}

/** Resumo de um changelog gravado na tag (o corpo em Markdown que o Warden monta). */
export interface ChangelogSummary {
  /** Primeira linha das notas do usuário, se houver. */
  notes: string | null;
  added: number;
  removed: number;
  updated: number;
  others: number;
  configs: number;
  platform: boolean;
}

const SECTION_TITLES: Record<string, keyof Omit<ChangelogSummary, 'notes'>> = {
  'Mods adicionados': 'added',
  'Mods removidos': 'removed',
  'Mods atualizados': 'updated',
  'Resource packs e shaders': 'others',
  'Configs alteradas': 'configs',
  'Minecraft e loader': 'platform',
};

/** Lê o changelog de uma versão: notas do usuário e quantos itens há em cada seção. */
export function summarizeChangelog(message: string): ChangelogSummary {
  const summary: ChangelogSummary = {
    notes: null,
    added: 0,
    removed: 0,
    updated: 0,
    others: 0,
    configs: 0,
    platform: false,
  };
  let section: keyof Omit<ChangelogSummary, 'notes'> | null = null;
  let inSections = false;
  for (const raw of message.replaceAll('\r\n', '\n').split('\n')) {
    const line = raw.trim();
    const heading = /^###\s+(.+)$/.exec(line);
    if (heading) {
      inSections = true;
      section = SECTION_TITLES[heading[1] ?? ''] ?? null;
      continue;
    }
    if (!inSections) {
      if (line !== '' && summary.notes === null) {
        summary.notes = line;
      }
      continue;
    }
    if (section !== null && line.startsWith('- ')) {
      if (section === 'platform') {
        summary.platform = true;
      } else {
        summary[section] += 1;
      }
    }
  }
  return summary;
}

/** A próxima versão de cada tamanho, para o exemplo do diálogo (`null` se não é X.Y.Z). */
export function nextVersions(
  last: string | null,
): { patch: string; minor: string; major: string } | null {
  const match = last === null ? null : /^(\d+)\.(\d+)\.(\d+)$/.exec(last);
  if (!match) {
    return null;
  }
  const [major, minor, patch] = [Number(match[1]), Number(match[2]), Number(match[3])];
  return {
    patch: [major, minor, patch + 1].join('.'),
    minor: [major, minor + 1, 0].join('.'),
    major: [major + 1, 0, 0].join('.'),
  };
}

/** Fuso do computador em minutos a leste do UTC (o `Date` do JS conta a oeste). */
export function utcOffsetMinutes(date: Date = new Date()): number {
  const west = date.getTimezoneOffset();
  return west === 0 ? 0 : -west;
}

/** Uma data como a linha do tempo mostra: "hoje, 15:02", "ontem, 15:02" ou "28/09/2026, 15:02". */
export interface WhenParts {
  relation: DayRelation;
  time: string;
  date: string;
}

/** Data e hora de um instante RFC 3339 (`null` se o texto não é uma data). */
export function whenParts(iso: string, now: number = Date.now()): WhenParts | null {
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) {
    return null;
  }
  return { relation: dayRelation(ms, now), time: formatTime(ms), date: formatDate(ms) };
}

/** Um nome seguro para o `id` de um elemento a partir de um número de versão. */
export function versionDomId(version: string): string {
  return `versao-${version.replaceAll(/[^0-9A-Za-z]+/g, '-')}`;
}
