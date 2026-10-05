/**
 * Lista de permissão do conteúdo remoto (descrições de mods do Modrinth e da CurseForge,
 * changelogs), comum ao `SafeMarkdown` (rehype-sanitize) e ao `SafeHtml` (DOMPurify)
 * (ARCHITECTURE §18 e §20).
 *
 * Permitido: títulos, parágrafos, listas, tabelas, `a`, `img`, `code`/`pre`, `details`/`summary`
 * e o atributo `align`, mais a formatação de texto que o próprio Markdown gera (negrito,
 * itálico, riscado, citação, quebra de linha). Proibido: `script`, `style`, atributos `on*`,
 * `form`, `object`/`embed`, `iframe` e qualquer outro elemento. Links e imagens só com
 * `https:` (imagens também `warden-img:`, o esquema das imagens da CurseForge, §17.1).
 *
 * Vídeos incorporados (`iframe` com `https:`) viram uma marca `div[data-warden-embed]` com o
 * endereço; o componente desenha a miniatura com "Abrir no navegador".
 */
import type { Element, ElementContent, Root, RootContent } from 'hast';
import type { Options as SanitizeSchema } from 'rehype-sanitize';

import { safeExternalUrl } from './external';

/** Elementos permitidos. */
export const ALLOWED_TAGS = [
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'p',
  'br',
  'hr',
  'ul',
  'ol',
  'li',
  'table',
  'thead',
  'tbody',
  'tfoot',
  'tr',
  'th',
  'td',
  'a',
  'img',
  'code',
  'pre',
  'details',
  'summary',
  'strong',
  'b',
  'em',
  'i',
  'u',
  's',
  'del',
  'blockquote',
  'sub',
  'sup',
  'kbd',
  'div',
  'span',
] as const;

/** Atributo que marca um vídeo incorporado (o valor é o endereço do vídeo). */
export const EMBED_ATTRIBUTE = 'data-warden-embed';

/** Esquemas permitidos em `href` e `src`. */
export const HREF_PROTOCOLS = ['https'] as const;
export const SRC_PROTOCOLS = ['https', 'warden-img'] as const;

/** Esquema do `rehype-sanitize` com a lista acima (sem `clobber` de ids: não há ids). */
export const sanitizeSchema: SanitizeSchema = {
  tagNames: [...ALLOWED_TAGS],
  attributes: {
    a: ['href', 'title'],
    img: ['src', 'alt', 'title', 'width', 'height', 'align'],
    details: ['open'],
    div: ['align', 'dataWardenEmbed'],
    p: ['align'],
    h1: ['align'],
    h2: ['align'],
    h3: ['align'],
    h4: ['align'],
    h5: ['align'],
    h6: ['align'],
    td: ['align'],
    th: ['align'],
    tr: ['align'],
    table: ['align'],
  },
  protocols: {
    href: [...HREF_PROTOCOLS],
    src: [...SRC_PROTOCOLS],
  },
  strip: ['script', 'style', 'template', 'noscript'],
  allowComments: false,
  allowDoctypes: false,
};

/** Endereço `https:` de um `iframe`, ou `null`. */
export function embedSource(src: unknown): string | null {
  if (typeof src !== 'string') {
    return null;
  }
  return safeExternalUrl(src)?.href ?? null;
}

function embedElement(src: string): Element {
  return { type: 'element', tagName: 'div', properties: { dataWardenEmbed: src }, children: [] };
}

function transform(nodes: (RootContent | ElementContent)[]): void {
  nodes.forEach((node, index) => {
    if (node.type !== 'element') {
      return;
    }
    if (node.tagName === 'iframe') {
      const src = embedSource(node.properties.src);
      // Sem endereço seguro, o iframe some (o sanitize remove o que sobrar).
      nodes[index] = src ? embedElement(src) : { type: 'text', value: '' };
      return;
    }
    transform(node.children);
  });
}

/** Plugin do rehype que troca `iframe` pela marca de vídeo. Roda antes do `rehype-sanitize`. */
export function rehypeEmbeds() {
  return (tree: Root) => {
    transform(tree.children);
  };
}

/** Miniatura de um vídeo conhecido (YouTube), ou `null`. */
export function videoThumbnail(src: string): string | null {
  const url = safeExternalUrl(src);
  if (!url) {
    return null;
  }
  const host = url.hostname.replace(/^www\./, '');
  let id: string | null = null;
  if (host === 'youtube.com' || host === 'youtube-nocookie.com') {
    const match = /^\/embed\/([\w-]{6,})/.exec(url.pathname);
    id = match?.[1] ?? url.searchParams.get('v');
  } else if (host === 'youtu.be') {
    id = url.pathname.slice(1) || null;
  }
  return id && /^[\w-]+$/.test(id) ? `https://img.youtube.com/vi/${id}/hqdefault.jpg` : null;
}

/** Nome do site de um endereço, para o texto da miniatura. */
export function hostOf(src: string): string {
  return safeExternalUrl(src)?.hostname.replace(/^www\./, '') ?? '';
}
