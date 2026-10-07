/**
 * Higienização de HTML remoto (descrições da CurseForge) com DOMPurify e a mesma lista de
 * permissão do Markdown (`lib/sanitize.ts`; ARCHITECTURE §18).
 *
 * Antes do DOMPurify, cada `iframe` com endereço `https:` vira `div[data-warden-embed]` (a
 * miniatura com "Abrir no navegador"); os demais somem. O `DOMParser` só monta a árvore, sem
 * executar scripts nem carregar nada.
 */
import DOMPurify from 'dompurify';

import { imageSrc } from './ipc/image-src';
import { ALLOWED_TAGS, EMBED_ATTRIBUTE, embedSource } from './sanitize';

/** Servidores de imagem da CurseForge: passam pelo protocolo `warden-img://` (só memória). */
const CURSEFORGE_IMAGE_HOSTS = ['forgecdn.net', 'curseforge.com'];

/**
 * O endereço pelo protocolo `warden-img://` de uma imagem da CurseForge, para o cache de disco
 * do WebView não guardar dados da API (ARCHITECTURE §17.1). Imagens de outros servidores (e
 * qualquer coisa que não seja `https:`) ficam como estão: o proxy do Rust recusaria.
 */
function proxiedImage(src: string): string | null {
  let url: URL;
  try {
    url = new URL(src.trim());
  } catch {
    return null;
  }
  const host = url.hostname.toLowerCase();
  const fromCurseForge = CURSEFORGE_IMAGE_HOSTS.some(
    (known) => host === known || host.endsWith(`.${known}`),
  );
  if (url.protocol !== 'https:' || !fromCurseForge) return null;
  const bytes = new TextEncoder().encode(url.href);
  const token = btoa(String.fromCharCode(...bytes))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/, '');
  return imageSrc(`warden-img://localhost/${token}`);
}

const ALLOWED_ATTR = [
  'href',
  'title',
  'src',
  'alt',
  'width',
  'height',
  'align',
  'open',
  EMBED_ATTRIBUTE,
];

/** Só `https:` em links e `https:`/`warden-img:` em imagens (conferido por atributo abaixo). */
const ALLOWED_URI_REGEXP = /^(?:https:|warden-img:)/i;

let purifier: ReturnType<typeof DOMPurify> | null = null;

function getPurifier(): ReturnType<typeof DOMPurify> {
  if (purifier) {
    return purifier;
  }
  const instance = DOMPurify(window);
  instance.addHook('uponSanitizeAttribute', (_node, data) => {
    // `warden-img:` só vale em imagem; link só com `https:`.
    if (data.attrName === 'href' && !/^https:/i.test(data.attrValue)) {
      data.keepAttr = false;
    }
  });
  // Depois da conferência dos atributos: o endereço do proxy não é `https:`.
  instance.addHook('afterSanitizeAttributes', (node) => {
    if (node.tagName === 'IMG') {
      const proxied = proxiedImage(node.getAttribute('src') ?? '');
      if (proxied) node.setAttribute('src', proxied);
    }
  });
  purifier = instance;
  return instance;
}

function replaceIframes(html: string): string {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html');
  for (const frame of Array.from(doc.querySelectorAll('iframe'))) {
    const src = embedSource(frame.getAttribute('src'));
    if (src) {
      const marker = doc.createElement('div');
      marker.setAttribute(EMBED_ATTRIBUTE, src);
      frame.replaceWith(marker);
    } else {
      frame.remove();
    }
  }
  return doc.body.innerHTML;
}

/** HTML higienizado, pronto para o `SafeHtml`. */
export function sanitizeHtml(html: string): string {
  return getPurifier().sanitize(replaceIframes(html), {
    ALLOWED_TAGS: [...ALLOWED_TAGS],
    ALLOWED_ATTR,
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
    ALLOWED_URI_REGEXP,
    // Com `ALLOWED_URI_REGEXP`, o DOMPurify confere contra ela todo atributo que não está na
    // lista dos "seguros"; estes não são endereços.
    ADD_URI_SAFE_ATTR: ['align', 'width', 'height', 'open'],
    FORBID_TAGS: ['style', 'script', 'form', 'object', 'embed', 'iframe'],
    KEEP_CONTENT: true,
  });
}
