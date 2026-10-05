import { describe, expect, it } from 'vitest';

import { embedSource, hostOf, videoThumbnail } from './sanitize';
import { sanitizeHtml } from './sanitize-html';

/** Conteúdo hostil típico de descrição de mod (OWASP XSS cheat sheet e afins). */
const HOSTILE = [
  '<script>alert(1)</script>',
  '<img src="https://cdn.modrinth.com/a.png" onerror="alert(1)">',
  '<a href="javascript:alert(1)">clique</a>',
  '<a href="data:text/html,<script>alert(1)</script>">dados</a>',
  '<a href="http://inseguro.com">http</a>',
  '<div style="background:url(javascript:alert(1))">estilo</div>',
  '<style>body{display:none}</style>',
  '<form action="https://x"><input name="senha"></form>',
  '<object data="https://x/y.swf"></object>',
  '<embed src="https://x/y.swf">',
  '<iframe src="javascript:alert(1)"></iframe>',
  '<svg onload="alert(1)"><circle /></svg>',
  '<math><mi xlink:href="javascript:alert(1)">x</mi></math>',
  '<p onclick="alert(1)" id="x" class="y">texto</p>',
  '<img src="file:///C:/Windows/win.ini">',
  '<base href="https://evil.example/">',
  '<meta http-equiv="refresh" content="0;url=https://evil.example">',
  '<template><script>alert(1)</script></template>',
].join('\n');

describe('sanitizeHtml (DOMPurify)', () => {
  const clean = sanitizeHtml(HOSTILE);

  it('remove script, style, form, object, embed, svg, math, base, meta e template', () => {
    for (const tag of [
      'script',
      'style',
      'form',
      'input',
      'object',
      'embed',
      'svg',
      'math',
      'base',
      'meta',
      'template',
      'iframe',
    ]) {
      expect(clean).not.toMatch(new RegExp(`<${tag}[\\s>]`, 'i'));
    }
  });

  it('remove todo atributo on*, style, id e class', () => {
    expect(clean).not.toMatch(/\son\w+=/i);
    expect(clean).not.toMatch(/\sstyle=/i);
    expect(clean).not.toMatch(/\sid=/i);
    expect(clean).not.toMatch(/\sclass=/i);
  });

  it('links só com https; o texto continua', () => {
    expect(clean).not.toMatch(/javascript:/i);
    expect(clean).not.toMatch(/href="data:/i);
    expect(clean).not.toMatch(/href="http:/i);
    expect(clean).toContain('clique');
    expect(clean).toContain('http');
  });

  it('imagem https fica; file: sai', () => {
    expect(clean).toContain('src="https://cdn.modrinth.com/a.png"');
    expect(clean).not.toContain('file:');
  });

  it('mantém a estrutura permitida (títulos, listas, tabela, details, align)', () => {
    const html = sanitizeHtml(
      '<h2 align="center">Recursos</h2><ul><li><strong>Rápido</strong></li></ul><table><tr><td>1</td></tr></table><details open><summary>Mais</summary>x</details><img src="warden-img://cf/1.png" alt="print">',
    );
    expect(html).toContain('<h2 align="center">Recursos</h2>');
    expect(html).toContain('<li><strong>Rápido</strong></li>');
    expect(html).toContain('<td>1</td>');
    expect(html).toContain('<details open="">');
    expect(html).toContain('src="warden-img://cf/1.png"');
  });

  it('warden-img só vale em imagem, não em link', () => {
    expect(sanitizeHtml('<a href="warden-img://x">x</a>')).toBe('<a>x</a>');
  });

  it('iframe https vira a marca de vídeo; iframe sem https some', () => {
    const html = sanitizeHtml(
      '<p>vídeo:</p><iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ" width="560"></iframe><iframe src="http://x"></iframe>',
    );
    expect(html).toBe(
      '<p>vídeo:</p><div data-warden-embed="https://www.youtube.com/embed/dQw4w9WgXcQ"></div>',
    );
  });
});

describe('vídeos incorporados', () => {
  it('endereço só com https', () => {
    expect(embedSource('https://youtube.com/embed/abc')).toBe('https://youtube.com/embed/abc');
    expect(embedSource('http://youtube.com/embed/abc')).toBeNull();
    expect(embedSource(42)).toBeNull();
  });

  it('miniatura do YouTube pelos três formatos de endereço', () => {
    const expected = 'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg';
    expect(videoThumbnail('https://www.youtube.com/embed/dQw4w9WgXcQ?start=3')).toBe(expected);
    expect(videoThumbnail('https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ')).toBe(expected);
    expect(videoThumbnail('https://www.youtube.com/watch?v=dQw4w9WgXcQ')).toBe(expected);
    expect(videoThumbnail('https://youtu.be/dQw4w9WgXcQ')).toBe(expected);
    expect(videoThumbnail('https://www.youtube.com/embed/<script>')).toBeNull();
    expect(videoThumbnail('https://vimeo.com/123')).toBeNull();
    expect(videoThumbnail('nada')).toBeNull();
  });

  it('nome do site sem www', () => {
    expect(hostOf('https://www.youtube.com/embed/x')).toBe('youtube.com');
    expect(hostOf('ftp://x')).toBe('');
  });
});
