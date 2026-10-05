/**
 * Higienização de HTML remoto (descrições da CurseForge) com DOMPurify e a mesma lista de
 * permissão do Markdown (`lib/sanitize.ts`; ARCHITECTURE §18).
 *
 * Antes do DOMPurify, cada `iframe` com endereço `https:` vira `div[data-warden-embed]` (a
 * miniatura com "Abrir no navegador"); os demais somem. O `DOMParser` só monta a árvore, sem
 * executar scripts nem carregar nada.
 */
import DOMPurify from 'dompurify';

import { ALLOWED_TAGS, EMBED_ATTRIBUTE, embedSource } from './sanitize';

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
