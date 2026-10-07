/**
 * Checklist "Antes de salvar" do diálogo Salvar versão (SPEC T16, P1): o diagnóstico sem erros,
 * o último teste depois da última mudança e as mudanças do teste revisadas. **Registro
 * acréscimo-apenas** (ROADMAP §1): cada tarefa dona de um desses fatos acrescenta uma linha
 * (D-03: erros do diagnóstico; L-04: último teste; C-03: mudanças do teste revisadas).
 *
 * Cada componente devolve um `<li>` com a classe `is-ok`, `is-bad` ou `is-warn` do `.checklist`.
 * O painel só aparece quando há ao menos um item registrado; a checklist nunca impede de salvar.
 */
import type { ComponentType } from 'react';

import type { PackId } from '../../lib/ipc/bindings';

export interface ChecklistItemProps {
  packId: PackId;
}

export interface ChecklistItem {
  /** Identificador estável (a tarefa dona troca a linha com o mesmo id). */
  id: string;
  component: ComponentType<ChecklistItemProps>;
}

/** Vazio até a D-03, a L-04 e a C-03 trazerem os fatos. */
export const checklistItems: readonly ChecklistItem[] = [];
