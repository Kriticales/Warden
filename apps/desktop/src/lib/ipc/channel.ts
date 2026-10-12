/**
 * Canal de uma operação longa (`Channel<OperationEvent>`; ARCHITECTURE §4.3): o comando recebe
 * o canal e manda por ele etapas, progresso, avisos e linhas de console enquanto roda.
 */
import { Channel } from '@tauri-apps/api/core';

import type { OperationEvent } from './bindings';

/** Um canal que entrega cada evento, na ordem, a `onEvent`. */
export function operationChannel(
  onEvent: (event: OperationEvent) => void,
): Channel<OperationEvent> {
  const channel = new Channel<OperationEvent>();
  channel.onmessage = onEvent;
  return channel;
}
