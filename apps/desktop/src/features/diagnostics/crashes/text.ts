/**
 * Textos de uma linha de Travamentos: a causa, as versões do pack e a situação.
 */
import i18n from '../../../i18n';
import type { CrashGroupView, CrashSituation, CrashVersion } from '../../../lib/ipc/bindings';
import { findingTitle } from '../text';

const t = i18n.getFixedT(null, 'travamentos');

/** Até quantas versões aparecem uma a uma ("1.4.1, 1.4.2 e 1.4.3"); mais que isso, a faixa. */
const MAX_LISTED_VERSIONS = 3;

/** Partes do sujeito da assinatura que dizem onde o jogo quebrou, na ordem de preferência. */
const WHERE_PARTS = ['entity_type=', 'block_entity=', 'description='];

/**
 * "Falta o mod Balm…" (o achado principal), "O jogo travou com NullPointerException
 * (alpha:golem)": sem o achado gravado, a exceção e onde quebrou (a entidade, o bloco ou a
 * descrição do crash report), para que duas causas diferentes não tenham o mesmo título.
 */
export function crashTitle(group: CrashGroupView): string {
  const primary = group.diagnosis?.findings[0];
  if (primary) {
    return findingTitle(primary);
  }
  const exception = group.signature.exception;
  if (exception) {
    const excecao = exception.split('.').pop() ?? exception;
    const onde = whereItBroke(group.signature.subject ?? []);
    return onde ? t('excecaoOnde', { excecao, onde }) : t('excecao', { excecao });
  }
  return t('semDetalhe');
}

function whereItBroke(subject: readonly string[]): string | undefined {
  for (const prefix of WHERE_PARTS) {
    const part = subject.find((value) => value.startsWith(prefix));
    if (part) {
      return part.slice(prefix.length);
    }
  }
  return undefined;
}

/** "1.4.2", "1.4.2 com alterações", "sem versão". */
export function versionLabel(version: CrashVersion): string {
  const name = version.version ?? t('semVersao');
  return version.unsaved ? t('comAlteracoes', { versao: name }) : name;
}

/** "versão 1.4.2", "versões 1.4.1 e 1.4.2", "versões 1.4.0 a 1.4.5". */
export function versionsText(versions: readonly CrashVersion[]): string {
  const labels = [...new Set(versions.map(versionLabel))];
  const first = labels[0];
  const last = labels[labels.length - 1];
  const versoes =
    labels.length > MAX_LISTED_VERSIONS && first !== undefined && last !== undefined
      ? t('faixa', { primeira: first, ultima: last })
      : new Intl.ListFormat('pt-BR', { type: 'conjunction' }).format(labels);
  return t('versao', { count: labels.length, versoes });
}

/** Tipo do selo da situação. */
export function situationKind(situation: CrashSituation): 'ok' | 'danger' | 'muted' {
  switch (situation.kind) {
    case 'resolved':
      return 'muted';
    case 'causeFound':
      return 'ok';
    case 'causeUnknown':
      return 'danger';
  }
}

export function situationText(situation: CrashSituation): string {
  switch (situation.kind) {
    case 'resolved':
      return t('situacao.resolvido', { versao: versionLabel(situation.since) });
    case 'causeFound':
      return t('situacao.causaEncontrada');
    case 'causeUnknown':
      return t('situacao.causaNaoEncontrada');
  }
}
