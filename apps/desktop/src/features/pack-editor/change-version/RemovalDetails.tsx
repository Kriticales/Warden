/**
 * Parte do diálogo de remover que vem do grafo da D-07 (SPEC T06; P1-15): a cadeia inteira do
 * que para de funcionar (quem depende de quem depende do item) e as dependências que ficam sem
 * uso, com uma caixa para cada uma. Nenhuma vem marcada: nada sai sem o usuário escolher.
 */
import { useTranslation } from 'react-i18next';

import type { RemovalPlan } from '../../../lib/ipc/bindings';
import { Checkbox } from '../../settings/components/fields';

export interface RemovalDetailsProps {
  plan: RemovalPlan;
  /** Caminhos das dependências sem uso marcadas para sair junto. */
  orphans: readonly string[];
  onOrphansChange: (paths: string[]) => void;
}

export function RemovalDetails({ plan, orphans, onOrphansChange }: RemovalDetailsProps) {
  const { t } = useTranslation('trocarVersao');
  return (
    <>
      {plan.dependents.length > 0 ? (
        <details className="disclosure" open={plan.dependents.some((d) => d.depth > 1)}>
          <summary>{t('remover.cadeiaTitulo')}</summary>
          <ul className="removelist" data-testid="cadeia">
            {plan.dependents.map((dependent) => (
              <li key={dependent.path}>
                {t('remover.precisaDe', {
                  name: dependent.name,
                  nomes: dependent.needs.join(', '),
                })}
                {dependent.depth > 1 ? (
                  <span className="t-3"> ({t('remover.indireto')})</span>
                ) : null}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
      {plan.orphans.length > 0 ? (
        <fieldset className="stack-2" data-testid="orfas">
          <legend className="field__label">
            {t('remover.orfasTitulo', { count: plan.orphans.length })}
          </legend>
          <p className="t-sm t-3">{t('remover.orfasTexto')}</p>
          {plan.orphans.map((orphan) => (
            <Checkbox
              key={orphan.path}
              label={orphan.name}
              desc={
                orphan.optionalUsers.length > 0
                  ? t('remover.usadaOpcional', { nomes: orphan.optionalUsers.join(', ') })
                  : undefined
              }
              checked={orphans.includes(orphan.path)}
              onChange={(checked) => {
                onOrphansChange(
                  checked
                    ? [...orphans, orphan.path]
                    : orphans.filter((path) => path !== orphan.path),
                );
              }}
            />
          ))}
        </fieldset>
      ) : null}
    </>
  );
}
