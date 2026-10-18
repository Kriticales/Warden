/**
 * Textos do "Testar como servidor" que dependem de valores (SPEC T27): os mods que ficam de fora,
 * o título do resultado e o que aconteceu em cada lado. Funções puras sobre o i18n.
 */
import i18n from '../../../i18n';
import type { ServerSide, TestSessionSummary } from '../../../lib/ipc/bindings';
import { formatDuration } from '../text';

const t = i18n.getFixedT(null, 'servidor');

/** O texto oficial da EULA (o mesmo do backend, `warden_server::EULA_URL`). */
export const EULA_URL = 'https://aka.ms/MinecraftEULA';

/** Quantos nomes a lista dos mods de fora mostra antes do "e mais N". */
const NAMES_SHOWN = 3;

/** "8 mods só de cliente ficam de fora: Embeddium, Rubidium, Jade e mais 5." */
export function clientOnlyText(names: readonly string[], count: number): string {
  if (count === 0) {
    return t('opcoes.nenhumDeFora');
  }
  const shown = names.slice(0, NAMES_SHOWN).join(', ');
  const rest = count - Math.min(names.length, NAMES_SHOWN);
  const nomes = rest > 0 ? t('opcoes.maisN', { nomes: shown, count: rest }) : shown;
  return t('opcoes.deFora', { count, nomes });
}

/**
 * O título do resultado de uma sessão com servidor, ou `null` (as outras ficam com o título do
 * Testar). O servidor que travou vem primeiro; em "Só o servidor", o título fala do servidor.
 */
export function serverResultTitle(session: TestSessionSummary): string | null {
  const side = session.server;
  if (session.mode !== 'server' || !side) {
    return null;
  }
  if (side.outcome === 'crashed') {
    return t('resultado.titulo.servidorTravou');
  }
  if (side.only) {
    return session.outcome === 'stoppedByUser'
      ? t('resultado.titulo.servidorEncerrado')
      : t('resultado.titulo.servidorFechou');
  }
  return null;
}

/** O que aconteceu com o servidor. */
export function serverSideText(side: ServerSide): string {
  const tempo = side.readyMs === null ? null : formatDuration(side.readyMs);
  if (side.outcome === 'crashed') {
    return tempo === null
      ? t('resultado.servidorTravouAntes')
      : t('resultado.servidorTravouDepois', { tempo });
  }
  if (tempo === null) {
    return t('resultado.servidorFechouAntes');
  }
  return side.stopForced
    ? t('resultado.servidorForcado', { tempo })
    : t('resultado.servidorAbriu', { tempo });
}

/** O que aconteceu com o jogo, visto do servidor. */
export function gameSideText(side: ServerSide): string {
  if (side.only) {
    return t('resultado.jogoSo');
  }
  switch (side.game) {
    case 'joined':
      return t('resultado.jogoEntrou');
    case 'opened':
      return t('resultado.jogoNaoEntrou');
    case 'notOpened':
      return t('resultado.jogoNaoAbriu');
  }
}
