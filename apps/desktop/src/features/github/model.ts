/**
 * Regras puras da publicação para os jogadores (SPEC T18): o texto do "Copiar instruções", a
 * separação entre bloqueios e avisos e a conferência do nome do repositório (a mesma regra do
 * backend, para avisar enquanto a pessoa digita).
 */
import type { TFunction } from 'i18next';

import type { PublishFinding } from '../../lib/ipc/bindings';

/** Comando do "antes de iniciar" do Prism/MultiMC. */
export function bootstrapCommand(packLink: string): string {
  return `"$INST_JAVA" -jar packwiz-installer-bootstrap.jar ${packLink}`;
}

export interface GameLabel {
  minecraft: string | null;
  loader: string | null;
}

/** Os quatro passos do jogador e a linha de ajuda do erro 403, como texto para copiar. */
export function playerInstructions(
  t: TFunction<'publicar'>,
  packLink: string,
  game: GameLabel,
): string {
  const minecraft = game.minecraft ?? '?';
  const first = game.loader
    ? t('passos.criar', { minecraft, loader: game.loader })
    : t('passos.criarSemLoader', { minecraft });
  return [
    t('passos.titulo'),
    '',
    `1. ${first}`,
    `2. ${t('passos.baixar')}`,
    `3. ${t('passos.comando')}`,
    `   ${bootstrapCommand(packLink)}`,
    `4. ${t('passos.abrir')}`,
    '',
    t('passos.ajuda403'),
  ].join('\n');
}

/** Achados que impedem publicar (segredo, higiene). */
export function isBlocking(finding: PublishFinding): boolean {
  return finding.kind === 'secretFound' || finding.kind === 'hygiene';
}

export function splitFindings(findings: readonly PublishFinding[]): {
  blocking: PublishFinding[];
  warnings: PublishFinding[];
} {
  return {
    blocking: findings.filter(isBlocking),
    warnings: findings.filter((finding) => !isBlocking(finding)),
  };
}

/** Nome de repositório que o GitHub aceita. */
export function isValidRepoName(name: string): boolean {
  return (
    name.length > 0 &&
    name.length <= 100 &&
    name !== '.' &&
    name !== '..' &&
    /^[A-Za-z0-9._-]+$/.test(name)
  );
}

/** `dono/nome` (ou o endereço do repositório no GitHub). */
export function isValidRepoRef(text: string): boolean {
  const path = text
    .trim()
    .replace(/^https:\/\/github\.com\//, '')
    .replace(/^github\.com\//, '')
    .replace(/\/$/, '')
    .replace(/\.git$/, '');
  const [owner, name, ...rest] = path.split('/');
  return (
    rest.length === 0 &&
    owner !== undefined &&
    /^[A-Za-z0-9-]{1,39}$/.test(owner) &&
    name !== undefined &&
    isValidRepoName(name)
  );
}
