/**
 * Markdown remoto seguro (ARCHITECTURE §18): `react-markdown` (com GFM: tabelas, riscado,
 * links automáticos) + `rehype-raw` **antes** do `rehype-sanitize` (o corpo do Modrinth
 * mistura Markdown e HTML; a ordem é obrigatória), com a lista de permissão de
 * `lib/sanitize.ts`. `iframe` vira miniatura com "Abrir no navegador"; links abrem no
 * navegador do sistema, só `https:`.
 */
import type { Element } from 'hast';
import type { ComponentPropsWithoutRef, MouseEvent } from 'react';
import Markdown, { type Components } from 'react-markdown';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import remarkGfm from 'remark-gfm';

import { cn } from '../../lib/cn';
import { openExternal, safeExternalUrl } from '../../lib/external';
import { rehypeEmbeds, sanitizeSchema } from '../../lib/sanitize';
import { VideoEmbed } from './VideoEmbed';

type WithNode<T extends keyof React.JSX.IntrinsicElements> = ComponentPropsWithoutRef<T> & {
  node?: Element | undefined;
};

/** As props sem o `node` do hast (que não é atributo HTML). */
function withoutNode<T extends { node?: Element | undefined }>(props: T): Omit<T, 'node'> {
  const copy = { ...props };
  delete copy.node;
  return copy;
}

function MarkdownLink(allProps: WithNode<'a'>) {
  const { href, children, ...props } = withoutNode(allProps);
  const url = safeExternalUrl(href);
  if (!url) {
    return <span>{children}</span>;
  }
  return (
    <a
      {...props}
      href={url.href}
      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault();
        void openExternal(url.href);
      }}
    >
      {children}
    </a>
  );
}

/** Imagem só com endereço absoluto permitido (o sanitize aceita endereços relativos). */
function MarkdownImage(allProps: WithNode<'img'>) {
  const { src, alt, ...props } = withoutNode(allProps);
  if (typeof src !== 'string' || !/^(?:https:|warden-img:)/i.test(src)) {
    return null;
  }
  return <img {...props} src={src} alt={alt ?? ''} loading="lazy" />;
}

function MarkdownDiv(allProps: WithNode<'div'>) {
  const embed = allProps.node?.properties.dataWardenEmbed;
  const { children, ...props } = withoutNode(allProps);
  if (typeof embed === 'string') {
    return <VideoEmbed src={embed} />;
  }
  return <div {...props}>{children}</div>;
}

const components: Components = {
  a: MarkdownLink,
  img: MarkdownImage,
  div: MarkdownDiv,
};

export interface SafeMarkdownProps {
  /** Texto Markdown (pode conter HTML). */
  children: string;
  className?: string | undefined;
}

export function SafeMarkdown({ children, className }: SafeMarkdownProps) {
  return (
    <div className={cn('prose', className)}>
      <Markdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw, rehypeEmbeds, [rehypeSanitize, sanitizeSchema]]}
        components={components}
        skipHtml={false}
      >
        {children}
      </Markdown>
    </div>
  );
}
