/**
 * Nomes das partes do jogo no raio-x: a classe sem o pacote e o método, com os nomes legíveis
 * quando algum refmap do pack os ensinou (`SodiumGameOptionPages.general`). O mesmo rótulo que o
 * aviso de Problemas usa (`Overlap::label` na `warden-mixin`).
 */
import type { ChangeKind, PointRow } from '../../../lib/ipc/bindings';

interface Named {
  targetClass: string;
  classNamed: string;
  method: string;
  methodNamed: string;
}

/** `net/minecraft/client/X` + `render` → `X.render`. */
export function partLabel(part: Named): string {
  const cls = part.classNamed || part.targetClass;
  const simple = cls.split('/').pop() ?? cls;
  return `${simple}.${part.methodNamed || part.method}`;
}

/** A classe completa, com pontos (`net.minecraft.client.X`). */
export function fullClass(part: Named): string {
  return (part.classNamed || part.targetClass).replaceAll('/', '.');
}

/** `ModifyArg` → `@ModifyArg`. */
export function annotation(kind: ChangeKind): string {
  return `@${kind.charAt(0).toUpperCase()}${kind.slice(1)}`;
}

/** As linhas que passam no filtro de texto (na parte do jogo ou na classe completa). */
export function matchesText(point: PointRow, text: string): boolean {
  const needle = text.trim().toLowerCase();
  if (!needle) return true;
  return [partLabel(point), fullClass(point), point.targetClass, point.method]
    .join(' ')
    .toLowerCase()
    .includes(needle);
}
