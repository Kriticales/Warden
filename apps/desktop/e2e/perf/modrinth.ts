/**
 * O Modrinth simulado para os itens do pack grande dos E2E de desempenho (A-04): os IDs são
 * inventados (`comum.ts`) e o servidor de fixtures (`mock-server/server.ts`) responde
 * `GET /projects?ids=` e `GET /versions?ids=` com o que sai daqui, junto das respostas
 * gravadas. Assim a primeira abertura do pack enche o cache de metadados do app, como no uso
 * de verdade, e as aberturas seguintes medem a meta "com cache" da SPEC §8.
 *
 * Os itens não têm ícone: nenhum pedido sai para fora do servidor de fixtures.
 */
/** Versão do Minecraft do pack grande. */
export const MINECRAFT = '1.20.1';

/** Pasta do item no pack → letra dos IDs inventados. */
const KINDS = {
  mods: { letter: 'm', projectType: 'mod', loaders: ['fabric'] },
  resourcepacks: { letter: 'r', projectType: 'resourcepack', loaders: ['minecraft'] },
  shaderpacks: { letter: 's', projectType: 'shader', loaders: ['iris'] },
} as const;

export type PerfFolder = keyof typeof KINDS;

/** O ID inventado do projeto do item `n` da pasta. */
export function perfProjectId(folder: PerfFolder, n: number): string {
  return `W${KINDS[folder].letter}${String(n).padStart(6, '0')}`;
}

/** O ID inventado da versão do item `n` da pasta. */
export function perfVersionId(folder: PerfFolder, n: number): string {
  return `V${KINDS[folder].letter}${String(n).padStart(6, '0')}`;
}

const ID = /^([WV])([mrs])(\d{6})$/;

function kindOf(letter: string) {
  return Object.values(KINDS).find((kind) => kind.letter === letter);
}

/**
 * Os projetos (`endpoint = 'modrinth/v2/projects'`) ou as versões (`'modrinth/v2/versions'`)
 * inventados entre os IDs pedidos; os outros IDs ficam para as respostas gravadas.
 */
export function perfModrinthItems(endpoint: string, wanted: readonly string[]): unknown[] {
  const prefix =
    endpoint === 'modrinth/v2/projects' ? 'W' : endpoint === 'modrinth/v2/versions' ? 'V' : null;
  if (!prefix) return [];
  const items: unknown[] = [];
  for (const id of wanted) {
    const [, side, letter = '', digits = ''] = ID.exec(id) ?? [];
    const kind = kindOf(letter);
    if (side !== prefix || !kind) continue;
    const n = Number(digits);
    const project = `W${kind.letter}${digits}`;
    if (prefix === 'W') {
      items.push({
        id,
        slug: `bench-${kind.projectType}-${String(n)}`,
        project_type: kind.projectType,
        title: `Item de Bancada ${kind.projectType} ${String(n)}`,
        description: 'Item gerado para medir o desempenho com um pack grande (A-04).',
        status: 'approved',
        updated: '2026-10-01T00:00:00Z',
        game_versions: [MINECRAFT],
        loaders: kind.loaders,
        client_side: 'required',
        server_side: kind.projectType === 'mod' ? 'required' : 'unsupported',
        versions: [`V${kind.letter}${digits}`],
        icon_url: null,
      });
    } else {
      items.push({
        id,
        project_id: project,
        name: `1.0.${String(n)}`,
        version_number: `1.0.${String(n)}`,
        version_type: 'release',
        game_versions: [MINECRAFT],
        loaders: kind.loaders,
        status: 'listed',
        date_published: '2026-10-01T00:00:00Z',
        changelog: '',
        files: [],
        dependencies: [],
      });
    }
  }
  return items;
}
