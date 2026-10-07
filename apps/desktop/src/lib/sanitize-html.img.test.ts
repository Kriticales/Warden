/**
 * Imagens da CurseForge na descrição em HTML passam pelo protocolo `warden-img://` (SPEC T08,
 * ARCHITECTURE §17.1): o WebView não guarda os dados da API no cache de disco. Imagens de outros
 * servidores e endereços sem `https:` não são mexidos.
 * @vitest-environment jsdom
 */
import { describe, expect, it } from 'vitest';

import { sanitizeHtml } from './sanitize-html';

function token(url: string): string {
  return btoa(url).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

describe('imagens da CurseForge na descrição', () => {
  it('forgecdn.net e curseforge.com viram warden-img, com o endereço original no token', () => {
    const url = 'https://media.forgecdn.net/attachments/1/2/captura.png';
    const html = sanitizeHtml(`<p><img src="${url}" alt="captura"></p>`);
    expect(html).toContain(`warden-img://localhost/${token(url)}`);
    expect(html).not.toContain('forgecdn.net');
    const other = 'https://media.curseforge.com/x.jpg';
    expect(sanitizeHtml(`<img src="${other}" alt="">`)).toContain(token(other));
  });

  it('outros servidores continuam diretos; http e javascript saem; servidor parecido não engana', () => {
    expect(sanitizeHtml('<img src="https://i.imgur.com/a.png" alt="">')).toContain(
      'https://i.imgur.com/a.png',
    );
    expect(sanitizeHtml('<img src="http://media.forgecdn.net/a.png" alt="">')).not.toContain(
      'src=',
    );
    expect(sanitizeHtml('<img src="javascript:alert(1)" alt="">')).not.toContain('javascript');
    const lookalike = 'https://evil-forgecdn.net/a.png';
    const html = sanitizeHtml(`<img src="${lookalike}" alt="">`);
    expect(html).toContain(lookalike);
    expect(html).not.toContain('warden-img');
  });

  it('o protocolo vale só em imagem, nunca em link', () => {
    const html = sanitizeHtml(
      '<a href="https://media.forgecdn.net/a.png"><img src="https://media.forgecdn.net/a.png" alt=""></a>',
    );
    expect(html).toContain('href="https://media.forgecdn.net/a.png"');
    expect(html).toContain('src="warden-img://localhost/');
  });
});
