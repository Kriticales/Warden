/**
 * Os sites de colagem dos E2E da conversa sem pack (D-19): o texto puro do mclo.gs
 * (`/mclogs/1/raw/<id>`, como `api.mclo.gs/1/raw/<id>`) e do pastebin (`/pastebin/raw/<id>`,
 * como `pastebin.com/raw/<id>`). O app aponta `WARDEN_API_BASE_MCLOGS` e
 * `WARDEN_API_BASE_PASTEBIN` para cá. O log tem dados pessoais de propósito: o teste confere
 * que nenhum deles chega à tela nem ao Gemini. Um id desconhecido responde 404, como o mclo.gs.
 */
import type { ServerResponse } from 'node:http';

/** O id do log simulado no mclo.gs. */
export const MCLOGS_E2E_ID = 'e2eLog123';
/** O id do texto simulado no pastebin. */
export const PASTEBIN_E2E_ID = 'E2ePaste';

/** Um crash com caminho de usuário e IP (a redação tem de tirar os dois). */
export const PASTE_E2E_LOG = [
  '[21:10:02] [main/INFO]: --gameDir C:\\Users\\Maria\\AppData\\Roaming\\x',
  '[21:11:41] [Server thread/INFO]: conectado de 192.168.0.42',
  '[21:14:07] [Server thread/ERROR]: java.lang.NoSuchMethodError: getPositionVec',
  '',
].join('\n');

/** Atende `/mclogs/…` e `/pastebin/…`. Devolve `false` se o caminho não é destes sites. */
export function servePaste(path: string, response: ServerResponse): boolean {
  const route = path.split('?')[0] ?? '';
  const text: Record<string, string> = {
    [`/mclogs/1/raw/${MCLOGS_E2E_ID}`]: PASTE_E2E_LOG,
    [`/pastebin/raw/${PASTEBIN_E2E_ID}`]: PASTE_E2E_LOG,
  };
  if (!route.startsWith('/mclogs/') && !route.startsWith('/pastebin/')) return false;
  const body = text[route];
  if (body === undefined) {
    response
      .writeHead(404, { 'content-type': 'application/json' })
      .end(JSON.stringify({ success: false, error: 'Log not found.' }));
  } else {
    response.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' }).end(body);
  }
  return true;
}
