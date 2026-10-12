/**
 * O que o texto do campo único de Adicionar é (SPEC T08 "Link"; P1-11): o mesmo formato que o
 * backend reconhece (`warden-project::add::url::classify_link`), só para a tela saber qual
 * painel abrir sem esperar. Quem decide de verdade é o backend, que confere de novo cada link.
 *
 * `http://` é recusado aqui mesmo, sem nenhum comando (CA-T08-07).
 */
import { parseCurseforgeLink } from '../curseforge/link';

export type LinkKind = 'modrinth' | 'curseforge' | 'direct' | 'insecure' | 'unsupported';

const MODRINTH_HOSTS = new Set(['modrinth.com', 'www.modrinth.com']);
const MODRINTH_TYPES = new Set([
  'mod',
  'plugin',
  'datapack',
  'resourcepack',
  'shader',
  'modpack',
  'project',
]);
const MAX_LINK_CHARS = 2048;
const SEGMENT = /^[A-Za-z0-9._+%-]{1,128}$/;

function hasScheme(text: string, scheme: string): boolean {
  return text.slice(0, scheme.length + 3).toLowerCase() === `${scheme}://`;
}

function validSegment(segment: string | undefined): segment is string {
  return segment !== undefined && SEGMENT.test(segment) && segment !== '.' && segment !== '..';
}

function isModrinth(url: URL): boolean {
  const segments = url.pathname.split('/').filter(Boolean);
  const host = url.hostname.toLowerCase();
  if (MODRINTH_HOSTS.has(host)) {
    const [type, project] = segments;
    return type !== undefined && MODRINTH_TYPES.has(type) && validSegment(project);
  }
  if (host === 'cdn.modrinth.com') {
    const [data, project, versions, version, file, ...rest] = segments;
    return (
      data === 'data' &&
      versions === 'versions' &&
      validSegment(project) &&
      validSegment(version) &&
      file !== undefined &&
      rest.length === 0
    );
  }
  return false;
}

/** O nome do arquivo de um link direto (último trecho do caminho, decodificado), ou `null`. */
export function directFileName(url: URL): string | null {
  let name: string;
  try {
    name = decodeURIComponent(url.pathname.replace(/\/+$/, '').split('/').pop() ?? '');
  } catch {
    return null;
  }
  if (!/\.(jar|zip)$/i.test(name)) return null;
  // Os mesmos caracteres que o backend recusa num nome de arquivo do pack.
  if (name.startsWith('.') || /[\\/:*?"<>|]/.test(name) || /\p{Cc}/u.test(name)) return null;
  return name;
}

/** `null`: não é link (o texto é uma busca pelo nome). */
export function classifyLink(text: string): LinkKind | null {
  const trimmed = text.trim();
  if (hasScheme(trimmed, 'http')) return 'insecure';
  if (!hasScheme(trimmed, 'https')) return trimmed.includes('://') ? 'unsupported' : null;
  if (trimmed.length > MAX_LINK_CHARS) return 'unsupported';
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return 'unsupported';
  }
  if (url.username !== '' || url.password !== '') return 'unsupported';
  if (parseCurseforgeLink(trimmed) !== null) return 'curseforge';
  if (isModrinth(url)) return 'modrinth';
  return directFileName(url) === null ? 'unsupported' : 'direct';
}

/** O servidor do link, para "Baixado de files.example.com". */
export function linkHost(text: string): string {
  try {
    return new URL(text.trim()).hostname;
  } catch {
    return '';
  }
}
