/**
 * HTML remoto seguro (ARCHITECTURE §18 e §20). Único lugar do app com
 * `dangerouslySetInnerHTML` (o ESLint só libera este arquivo), e só com o resultado de
 * `sanitizeHtml` (DOMPurify com a lista de permissão comum).
 *
 * Cliques em links não navegam a janela: abrem no navegador do sistema, só `https:`. Vídeos
 * incorporados viram a miniatura com "Abrir no navegador" (renderizada por portal no lugar da
 * marca `div[data-warden-embed]`).
 */
import { useLayoutEffect, useMemo, useRef, useState, type MouseEvent } from 'react';
import { createPortal } from 'react-dom';

import { cn } from '../../lib/cn';
import { openExternal } from '../../lib/external';
import { EMBED_ATTRIBUTE } from '../../lib/sanitize';
import { sanitizeHtml } from '../../lib/sanitize-html';
import { VideoEmbed } from './VideoEmbed';

export interface SafeHtmlProps {
  /** HTML não confiável. */
  html: string;
  className?: string | undefined;
}

interface Embed {
  target: Element;
  src: string;
}

export function SafeHtml({ html, className }: SafeHtmlProps) {
  const clean = useMemo(() => sanitizeHtml(html), [html]);
  const container = useRef<HTMLDivElement>(null);
  const [embeds, setEmbeds] = useState<Embed[]>([]);

  useLayoutEffect(() => {
    const root = container.current;
    if (!root) {
      return;
    }
    const found = Array.from(root.querySelectorAll(`[${EMBED_ATTRIBUTE}]`)).flatMap((target) => {
      const src = target.getAttribute(EMBED_ATTRIBUTE);
      return src ? [{ target, src }] : [];
    });
    setEmbeds(found);
  }, [clean]);

  const onClick = (event: MouseEvent<HTMLDivElement>) => {
    if (!(event.target instanceof Element)) {
      return;
    }
    const link = event.target.closest('a[href]');
    if (link && container.current?.contains(link)) {
      event.preventDefault();
      void openExternal(link.getAttribute('href') ?? '');
    }
  };

  return (
    <>
      {/* O clique só intercepta links; o teclado chega pelo próprio <a> (Enter gera clique). */}
      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions */}
      <div
        ref={container}
        className={cn('prose', className)}
        onClick={onClick}
        dangerouslySetInnerHTML={{ __html: clean }}
      />
      {embeds.map((embed, index) =>
        createPortal(<VideoEmbed src={embed.src} />, embed.target, `embed-${String(index)}`),
      )}
    </>
  );
}
