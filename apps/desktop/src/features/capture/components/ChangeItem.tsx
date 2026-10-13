/**
 * Uma mudança com caixa de marcar (SPEC T15): o caminho, a diferença por valor (ou o que
 * aconteceu com o arquivo) e "Ver diferença" quando há texto dos dois lados.
 */
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';

export interface ChangeItemProps {
  /** O que a caixa marca (para o leitor de tela). */
  name: string;
  /** Texto principal (o caminho, ou a chave do `options.txt`). */
  label: ReactNode;
  checked: boolean;
  onCheck: (checked: boolean) => void;
  children?: ReactNode;
  /** "Ver diferença" (sem: o item não tem texto para comparar). */
  onDiff?: (() => void) | undefined;
}

export function ChangeItem({ name, label, checked, onCheck, children, onDiff }: ChangeItemProps) {
  const { t } = useTranslation('captura');
  return (
    <div className="change-item">
      <label className="check">
        <input
          type="checkbox"
          checked={checked}
          aria-label={t('item.trazer', { nome: name })}
          onChange={(event) => {
            onCheck(event.target.checked);
          }}
        />
      </label>
      <div className="change-item__body">
        <div className="t-sm t-strong path">{label}</div>
        {children ? <div className="change-item__detail">{children}</div> : null}
      </div>
      <div>
        {onDiff ? (
          <Button
            variant="link"
            aria-label={t('item.verDiferencaDe', { nome: name })}
            onClick={onDiff}
          >
            {t('item.verDiferenca')}
          </Button>
        ) : null}
      </div>
    </div>
  );
}
