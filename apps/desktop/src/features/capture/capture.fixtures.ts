/** Capturas de exemplo para os testes de componente (C-03). */
import type { CaptureFileView, CaptureView, CapturedFile, HashMatch } from '../../lib/ipc/bindings';

export const SESSION = '2026-10-07_14-20-00';

export function makeFile(overrides: Partial<CapturedFile> = {}): CapturedFile {
  return {
    path: 'config/sodium-options.json',
    group: 'packConfig',
    change: 'modified',
    selected: true,
    packPath: 'config/sodium-options.json',
    values: [{ key: 'quality.weather_quality', before: '"DEFAULT"', after: '"FANCY"' }],
    options: [],
    hiddenVolatile: 0,
    text: true,
    beforeSha256: 'aa',
    afterSha256: 'bb',
    size: 120,
    managed: true,
    ...overrides,
  };
}

export function fileView(
  file: Partial<CapturedFile> = {},
  overrides: Partial<Omit<CaptureFileView, 'file'>> = {},
): CaptureFileView {
  const made = makeFile(file);
  return { file: made, packSha256: `pack-${made.path}`, conflict: null, ...overrides };
}

export const OPTIONS = fileView({
  path: 'options.txt',
  group: 'options',
  packPath: 'options.txt',
  values: [],
  options: [
    { key: 'renderDistance', before: '12', after: '16', selected: true },
    { key: 'key_key.zoom', before: null, after: 'key.keyboard.c', selected: false },
  ],
  hiddenVolatile: 3,
});

export const JEI_MATCH: HashMatch = {
  source: 'modrinth',
  projectId: 'u6dRKJwZ',
  versionId: 'abc123',
  title: 'Just Enough Items',
  versionNumber: '15.20.0.112',
  kind: 'mod',
  compatible: true,
};

export function makeCapture(overrides: Partial<CaptureView> = {}): CaptureView {
  return {
    sessionId: SESSION,
    status: { kind: 'pending' },
    needsReview: true,
    reformatted: 2,
    files: [
      fileView(),
      OPTIONS,
      fileView({
        path: 'saves/Mundo de teste/serverconfig/create-server.toml',
        group: 'worldConfig',
        change: 'added',
        selected: false,
        packPath: 'defaultconfigs/create-server.toml',
        values: [],
        managed: false,
      }),
      fileView({
        path: 'mods/jei-1.20.1-forge-15.20.0.112.jar',
        group: 'mod',
        change: 'added',
        selected: false,
        packPath: null,
        values: [],
        text: false,
        managed: false,
      }),
      fileView({
        path: 'mods/desconhecido.jar',
        group: 'mod',
        change: 'added',
        selected: false,
        packPath: null,
        values: [],
        text: false,
        managed: false,
      }),
      fileView({
        path: 'config/jade/plugins.json',
        group: 'generated',
        change: 'added',
        selected: false,
        values: [],
        managed: false,
      }),
    ],
    ...overrides,
  };
}
