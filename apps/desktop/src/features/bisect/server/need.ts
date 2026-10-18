/**
 * Quando a busca do culpado precisa do servidor local (SPEC T25; decisão D18; ROADMAP D-13) e o
 * que o pedido leva com o aceite ou sem ele.
 *
 * - **Versão:** "Trava ao entrar no mundo" num Minecraft sem Quick Play para um mundo de um
 *   jogador (antes de 1.20): o jogo só entra no mundo por um servidor.
 * - **Servidor:** o travamento de origem foi no servidor ("Testar como servidor").
 *
 * Sem o aceite, a busca usa o modo assistido (a pessoa entra no mundo e responde).
 */
import type { BisectPlan, Symptom } from '../../../lib/ipc/bindings';

/** Por que a busca precisa do servidor local. */
export type ServerReason = 'version' | 'server';

/** Se a busca precisa do servidor local (e por quê); `null` quando não precisa. */
export function serverReason(
  plan: Pick<BisectPlan, 'quickPlay'>,
  kind: Symptom['kind'],
  fromServer: boolean,
): ServerReason | null {
  if (fromServer) return 'server';
  if (kind === 'crashOnWorld' && !plan.quickPlay) return 'version';
  return null;
}

/** O sintoma e o `server` do pedido: com o aceite, o servidor local; sem ele, o modo assistido. */
export function withServer(
  symptom: Symptom,
  reason: ServerReason | null,
  accepted: boolean,
): { symptom: Symptom; server: boolean } {
  if (reason === null) return { symptom, server: false };
  if (accepted) return { symptom, server: true };
  return { symptom: { kind: 'assisted' }, server: false };
}
