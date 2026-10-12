import { describe, expect, it } from 'vitest';

import type { FileReport, HashMatch } from '../../../lib/ipc/bindings';
import { makeAppError } from '../../../test/factories';
import { fileErrorReason, initialKind, loaderList, referenceMatch } from './model';

function report(overrides: Partial<FileReport> = {}): FileReport {
  return {
    fileName: 'sodium.jar',
    size: 1000,
    sha256: 'a'.repeat(64),
    kind: 'mod',
    archive: true,
    modInfo: null,
    incompatible: [],
    side: 'both',
    sideNote: null,
    matches: [],
    lookupFailed: [],
    inPack: null,
    ...overrides,
  };
}

const MATCH: HashMatch = {
  source: 'modrinth',
  projectId: 'AANobbMI',
  versionId: 'SMxNOGZ6',
  title: 'Sodium',
  versionNumber: '0.6.0',
  kind: 'mod',
  compatible: true,
};

describe('arquivo do computador', () => {
  it('lista de loaders em português', () => {
    expect(loaderList([])).toBe('');
    expect(loaderList(['fabric'])).toBe('Fabric');
    expect(loaderList(['fabric', 'quilt'])).toBe('Fabric e Quilt');
    expect(loaderList(['forge', 'neoforge', 'fabric'])).toBe('Forge, NeoForge e Fabric');
  });

  it('o achado para a referência ignora tipos que a página não aceita', () => {
    expect(referenceMatch(report())).toBeNull();
    const modpack = { ...MATCH, kind: null };
    expect(referenceMatch(report({ matches: [modpack] }))).toBeNull();
    const curseforge = { ...MATCH, source: 'curseforge' as const, projectId: '394468' };
    expect(referenceMatch(report({ matches: [modpack, MATCH, curseforge] }))).toBe(MATCH);
  });

  it('tipo inicial pelo conteúdo, pelo achado ou mod', () => {
    expect(initialKind(report({ kind: 'shader' }))).toBe('shader');
    expect(initialKind(report({ kind: null, matches: [{ ...MATCH, kind: 'resourcePack' }] }))).toBe(
      'resourcePack',
    );
    expect(initialKind(report({ kind: null }))).toBe('mod');
  });

  it('motivo dos erros de arquivo', () => {
    const error = makeAppError(
      { domain: 'project', code: 'INVALID_INPUT' },
      { params: { reason: 'nameTaken', name: 'a.jar' } },
    );
    expect(fileErrorReason(error)).toBe('nameTaken');
    expect(fileErrorReason({ ...error, params: { reason: 'outro' } })).toBeNull();
    expect(fileErrorReason(makeAppError())).toBeNull();
  });
});
