/**
 * Modelo dos padrões das configs (C-06): o que o formulário faz com o estado de cada chave
 * diante do padrão que o arquivo informa (camada A). Funções puras, sem IPC.
 *
 * Só o padrão escrito pelo arquivo (`Default:` do NeoForge, `[default: …]` do Forge antigo)
 * conta: o deduzido de uma frase do comentário aparece, mas nunca marca a chave como alterada
 * nem pode ser restaurado (o Rust já devolve `unknown` para ele).
 */
import type { ConfigField, FormItem } from '../../../../lib/ipc/bindings';

/** O padrão que o arquivo informa (não o deduzido). */
export function knownDefault(field: ConfigField): string | null {
  return field.default && !field.default.inferred ? field.default.value : null;
}

/** A chave está diferente do padrão que o arquivo informa. */
export function isChanged(field: ConfigField): boolean {
  return field.defaultState === 'changed';
}

/** "Restaurar padrão" serve: a chave está diferente de um padrão conhecido. */
export function canRestore(field: ConfigField): boolean {
  return isChanged(field) && knownDefault(field) !== null;
}

/** Quantas chaves do formulário estão diferentes do padrão. */
export function changedCount(items: readonly FormItem[]): number {
  return items.filter((item) => item.kind === 'field' && isChanged(item)).length;
}

/**
 * Só as chaves alteradas, cada uma com o título da seção dela (seções sem chave alterada e
 * trechos só em texto saem).
 */
export function onlyChanged(items: readonly FormItem[]): FormItem[] {
  const out: FormItem[] = [];
  let section: FormItem | null = null;
  for (const item of items) {
    if (item.kind === 'section') {
      section = item;
    } else if (item.kind === 'field' && isChanged(item)) {
      if (section) {
        out.push(section);
        section = null;
      }
      out.push(item);
    }
  }
  return out;
}
