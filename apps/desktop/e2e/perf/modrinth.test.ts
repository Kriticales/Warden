/**
 * O Modrinth simulado do pack grande (A-04): responde só os IDs inventados, no endpoint certo,
 * e deixa os outros para as respostas gravadas do servidor de fixtures.
 */
import { describe, expect, it } from 'vitest';

import { perfModrinthItems, perfProjectId, perfVersionId } from './modrinth';

describe('Modrinth simulado do pack grande', () => {
  it('projetos e versões só dos IDs inventados, cada um no seu endpoint', () => {
    const project = perfProjectId('mods', 7);
    const version = perfVersionId('shaderpacks', 12);
    expect(project).toBe('Wm000007');
    expect(version).toBe('Vs000012');

    const projects = perfModrinthItems('modrinth/v2/projects', [project, version, 'AANobbMI']);
    expect(projects).toEqual([
      expect.objectContaining({ id: 'Wm000007', project_type: 'mod', icon_url: null }),
    ]);
    const versions = perfModrinthItems('modrinth/v2/versions', [project, version]);
    expect(versions).toEqual([
      expect.objectContaining({
        id: 'Vs000012',
        project_id: 'Ws000012',
        version_number: '1.0.12',
        loaders: ['iris'],
      }),
    ]);
    expect(perfModrinthItems('modrinth/v2/search', [project])).toEqual([]);
  });
});
