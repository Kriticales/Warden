/**
 * A busca a partir de um "Testar como servidor" que travou no servidor (SPEC T25 e T27;
 * ROADMAP D-13): o travamento é do servidor, então a busca pede o servidor local; antes do
 * "Done" ele travou ao abrir, depois, ao entrar no mundo.
 */
import type { Symptom, TestSessionSummary } from '../../../lib/ipc/bindings';

/** `null` quando a sessão não travou no servidor. */
export function serverCrashOrigin(
  session: TestSessionSummary,
): { server: true; symptom: Symptom['kind'] } | null {
  const side = session.server;
  if (session.mode !== 'server' || side?.outcome !== 'crashed') return null;
  return { server: true, symptom: side.readyMs === null ? 'crashOnOpen' : 'crashOnWorld' };
}
