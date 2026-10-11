/**
 * Links da CurseForge no campo único de Adicionar (SPEC T08 "Link"): o mesmo formato que o
 * backend reconhece (`warden-project::add::link_curseforge::parse_link`), só para a tela saber
 * quando oferecer a leitura do link. Quem decide o que o link aponta é o backend.
 */
import type { LinkTarget, SearchResult } from '../../../lib/ipc/bindings';

/** O que o link é: de um projeto ou de um arquivo (`/files/<id>`, `/download/<id>`). */
export type CurseforgeLinkKind = 'project' | 'file';

const HOSTS = new Set(['www.curseforge.com', 'curseforge.com', 'legacy.curseforge.com']);
const CLASSES = new Set([
  'mc-mods',
  'texture-packs',
  'shaders',
  'modpacks',
  'customization',
  'data-packs',
  'worlds',
  'bukkit-plugins',
  'mc-addons',
]);

/** `null` se o texto não é um link `https://` de um projeto ou arquivo do Minecraft na CurseForge. */
export function parseCurseforgeLink(text: string): CurseforgeLinkKind | null {
  const trimmed = text.trim();
  if (!trimmed.startsWith('https://')) return null;
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return null;
  }
  if (!HOSTS.has(url.hostname.toLowerCase())) return null;
  const [game, section, slug, tab, id, ...rest] = url.pathname.split('/').filter(Boolean);
  if (game !== 'minecraft' || !section || !CLASSES.has(section) || !slug) return null;
  if (tab === undefined) return 'project';
  if ((tab === 'files' || tab === 'download') && id !== undefined && rest.length === 0) {
    return /^\d+$/.test(id) ? 'file' : null;
  }
  return 'project';
}

/**
 * O projeto de um link de projeto, no formato de um resultado da busca, para abrir a
 * pré-visualização (que lê o resto pela API: descrição, versões, ícone).
 */
export function resultOfLink(target: LinkTarget): SearchResult {
  return {
    key: `curseforge:${target.projectId}`,
    sources: [{ source: 'curseforge', projectId: target.projectId, slug: '', downloads: 0 }],
    title: target.title,
    author: '',
    summary: '',
    iconUrl: null,
    downloads: 0,
    updated: '',
    inPack: false,
    compatible: true,
    manualDownload: false,
  };
}
