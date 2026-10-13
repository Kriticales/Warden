import { describe, expect, it } from 'vitest';

import i18n from '../../i18n';
import {
  bootstrapCommand,
  isValidRepoName,
  isValidRepoRef,
  playerInstructions,
  splitFindings,
} from './model';

const LINK = 'https://raw.githubusercontent.com/dono/pack/main/pack.toml';

describe('publicação: regras puras', () => {
  it('comando do bootstrap e instruções para copiar', () => {
    expect(bootstrapCommand(LINK)).toBe(
      `"$INST_JAVA" -jar packwiz-installer-bootstrap.jar ${LINK}`,
    );
    const t = i18n.getFixedT('pt-BR', 'publicar');
    const text = playerInstructions(t, LINK, { minecraft: '1.21.1', loader: 'NeoForge 21.1.80' });
    expect(text).toContain('1. Criar uma instância com Minecraft 1.21.1 e NeoForge 21.1.80.');
    expect(text).toContain(`   ${bootstrapCommand(LINK)}`);
    expect(text).toContain('erro 403');
    expect(playerInstructions(t, LINK, { minecraft: '1.21.1', loader: null })).toContain(
      'Criar uma instância com Minecraft 1.21.1.',
    );
  });

  it('segredo e higiene bloqueiam; o resto é aviso', () => {
    const { blocking, warnings } = splitFindings([
      { kind: 'untested' },
      { kind: 'secretFound', path: 'config/a.toml', line: 1 },
      { kind: 'curseforgeOnly', mods: [] },
    ]);
    expect(blocking.map((f) => f.kind)).toEqual(['secretFound']);
    expect(warnings.map((f) => f.kind)).toEqual(['untested', 'curseforgeOnly']);
  });

  it('nomes de repositório como o GitHub aceita', () => {
    expect(isValidRepoName('vale-sereno_2.0')).toBe(true);
    for (const bad of ['', '.', '..', 'com espaço', 'ação', 'a'.repeat(101)]) {
      expect(isValidRepoName(bad)).toBe(false);
    }
    expect(isValidRepoRef('dono/pack')).toBe(true);
    expect(isValidRepoRef('https://github.com/dono/pack.git')).toBe(true);
    for (const bad of ['pack', 'dono/', 'a/b/c', 'dono do/pack']) {
      expect(isValidRepoRef(bad)).toBe(false);
    }
  });
});
