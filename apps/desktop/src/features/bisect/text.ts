/**
 * Textos da busca do culpado que dependem de valores (SPEC T25): o resultado de cada rodada
 * em ícone e palavra, o que cada rodada liga, "O que o Warden está fazendo agora", a
 * estimativa e o título do resultado. Funções puras sobre o i18n.
 */
import i18n from '../../i18n';
import type {
  BisectCurrent,
  BisectItem,
  RoundKind,
  BisectView,
  Estimate,
  Verdict,
} from '../../lib/ipc/bindings';

const t = i18n.getFixedT(null, 'buscaDoCulpado');

/** Como a trilha e a tabela mostram uma rodada (classe `.round--*` do design system). */
export type RoundMark = 'same' | 'pass' | 'diff' | 'timeout' | 'now' | 'ask' | 'paused' | 'todo';

/** A marca de um veredito. */
export function markOf(verdict: Verdict): RoundMark {
  switch (verdict.kind) {
    case 'passed':
    case 'userNo':
      return 'pass';
    case 'failedSame':
    case 'userYes':
      return 'same';
    case 'timeout':
      return 'timeout';
    case 'failedOther':
    case 'inferred':
    case 'environment':
    case 'userSkip':
      return 'diff';
  }
}

/** A palavra de uma marca. */
export function markWord(mark: RoundMark): string {
  return t(`marca.${mark}`);
}

/** A palavra do veredito, mais precisa que a da marca na tabela. */
export function verdictWord(verdict: Verdict): string {
  switch (verdict.kind) {
    case 'passed':
      return t('veredito.passou');
    case 'failedSame':
      return t('veredito.travouIgual');
    case 'failedOther':
      return t('veredito.travouDiferente');
    case 'timeout':
      return t('veredito.tempo');
    case 'environment':
      return t('veredito.ambiente');
    case 'inferred':
      return t('veredito.inferida');
    case 'userYes':
      return t('veredito.aconteceu');
    case 'userNo':
      return t('veredito.naoAconteceu');
    case 'userSkip':
      return t('veredito.naoDeu');
  }
}

/** Nomes juntos ("Epic Fight e Supplementaries"). */
export function joinNames(names: readonly string[]): string {
  return new Intl.ListFormat('pt-BR', { style: 'long', type: 'conjunction' }).format(names);
}

/** O que a rodada liga ("Primeiros 69 mods", "Sem mods", "Pack sem o Epic Fight"). */
export function roundWhat(kind: RoundKind, found: readonly string[]): string {
  const nomes = joinNames(found);
  switch (kind.kind) {
    case 'reproduce':
      return t('rodada.inteiro');
    case 'control':
      return t('rodada.controle');
    case 'prefix':
      return kind.withFound
        ? t('rodada.prefixoCom', { count: kind.size, nomes })
        : t('rodada.prefixo', { count: kind.size });
    case 'alone':
      return t('rodada.sozinho', { nomes });
    case 'without':
      return t('rodada.sem', { nomes });
  }
}

/** "O que o Warden está fazendo agora". */
export function nowText(current: BisectCurrent, searched: number): string {
  const rodada = current.number;
  const kind = current.kind;
  switch (kind.kind) {
    case 'reproduce':
      return t('agora.inteiro', { rodada, tentativa: kind.attempt });
    case 'control':
      return t('agora.controle', { rodada });
    case 'prefix': {
      const base = t('agora.prefixo', { rodada, ligados: current.enabledCount, total: searched });
      return current.added.length > 0
        ? `${base} ${t('agora.acrescentados', { count: current.added.length })}`
        : base;
    }
    case 'alone':
      return t('agora.sozinho', { rodada, nomes: joinNames(current.found) });
    case 'without':
      return t('agora.sem', { rodada, nomes: joinNames(current.found) });
  }
}

/** Minutos arredondados para cima ("12 min"; nunca para baixo, SPEC T25). */
export function minutesText(ms: number): string {
  return t('minutos', { count: Math.max(1, Math.ceil(ms / 60_000)) });
}

/** "11 a 19 rodadas · cerca de 17 a 29 minutos". */
export function estimateText(estimate: Estimate): string {
  const rodadas =
    estimate.roundsMin === estimate.roundsMax
      ? t('estimativa.rodadasUma', { count: estimate.roundsMin })
      : t('estimativa.rodadas', { min: estimate.roundsMin, max: estimate.roundsMax });
  const minutos =
    estimate.minutesMin === estimate.minutesMax
      ? t('estimativa.minutosUm', { count: estimate.minutesMax })
      : t('estimativa.minutos', { min: estimate.minutesMin, max: estimate.minutesMax });
  return `${rodadas} · ${minutos}`;
}

/** A linha abaixo do título durante a busca. */
export function progressLine(view: BisectView): string {
  const done = view.rounds.length;
  const suspeitos = view.suspects.length;
  const tempo = minutesText(view.elapsedMs);
  if (view.status === 'paused') {
    return t('linha.pausada', { rodada: done, count: suspeitos, tempo });
  }
  return t('linha.andamento', {
    rodada: view.current?.number ?? done,
    min: view.estimate.roundsMin,
    max: view.estimate.roundsMax,
    count: suspeitos,
    tempo,
  });
}

/** Nomes dos itens. */
export function namesOf(items: readonly BisectItem[]): string[] {
  return items.map((item) => item.name);
}
