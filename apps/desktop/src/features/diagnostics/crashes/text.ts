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

/** Tamanho máximo de "onde quebrou" no título, com as reticências. */
export const MAX_WHERE_LENGTH = 80;

/** Frame nativo da JVM (`C ig9icd64.dll`): a letra do tipo sai do título. */
const NATIVE_KIND = /^[CJjVv] /;

/**
 * "Falta o mod Balm…" (o achado principal), "O jogo travou com NullPointerException
 * (alpha:golem em EntityTicker.tick)": sem o achado gravado, a exceção e onde quebrou (a
 * entidade, o bloco ou a descrição do crash report, e o frame da assinatura), para que duas
 * causas diferentes não tenham o mesmo título.
 */
export function crashTitle(group: CrashGroupView): string {
  const primary = group.diagnosis?.findings[0];
  if (primary) {
    return findingTitle(primary);
  }
  const exception = group.signature.exception;
  if (exception) {
    const excecao = exception.split('.').pop() ?? exception;
    const onde = whereItBroke(group.signature.subject ?? [], group.signature.frame);
    return onde ? t('excecaoOnde', { excecao, onde }) : t('excecao', { excecao });
  }
  return t('semDetalhe');
}

/** Menor pedaço da descrição que vale manter antes do frame, ao cortar. */
const MIN_WHAT_LENGTH = 12;

/**
 * "alpha:golem em EntityTicker.tick", em até {@link MAX_WHERE_LENGTH} caracteres. Para caber,
 * corta primeiro o que quebrou e deixa o frame, que é o que separa as causas.
 */
function whereItBroke(subject: readonly string[], frame: string | null): string | undefined {
  const what = whatBroke(subject);
  const place = frame ? shortFrame(frame) : undefined;
  if (what && place) {
    const full = t('ondeEm', { onde: what, frame: place });
    const room = MAX_WHERE_LENGTH - (full.length - what.length);
    if (full.length <= MAX_WHERE_LENGTH || room < MIN_WHAT_LENGTH) {
      return shorten(full);
    }
    return t('ondeEm', { onde: shorten(what, room), frame: place });
  }
  if (place) {
    return shorten(t('em', { frame: place }));
  }
  return what ? shorten(what) : undefined;
}

function whatBroke(subject: readonly string[]): string | undefined {
  for (const prefix of WHERE_PARTS) {
    const part = subject.find((value) => value.startsWith(prefix));
    if (part) {
      return part.slice(prefix.length);
    }
  }
  return undefined;
}

/** Só a classe e o método: `net.minecraft.client.renderer.GameRenderer.render` → `GameRenderer.render`. */
function shortFrame(frame: string): string {
  const place = frame.replace(NATIVE_KIND, '');
  return place.split('.').slice(-2).join('.');
}

/**
 * O texto até `max` caracteres, com reticências. Corta no último espaço (entre nomes) quando ele
 * fica na segunda metade; senão, no meio do nome.
 */
export function shorten(text: string, max = MAX_WHERE_LENGTH): string {
  if (text.length <= max) {
    return text;
  }
  const room = text.slice(0, max - 1);
  const space = room.lastIndexOf(' ');
  const cut = space >= max / 2 ? room.slice(0, space) : room;
  return `${cut.trimEnd()}…`;
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
