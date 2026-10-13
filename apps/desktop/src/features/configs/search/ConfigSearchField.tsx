/**
 * O campo "Buscar em todas as configs" (SPEC T12; protótipo `configs-busca`), acima da árvore.
 * Ao aparecer, prepara o índice da origem (a leitura em segundo plano começa antes da primeira
 * tecla) e passa a ouvir `config-index-changed`. Esc limpa a busca.
 */
import { Search, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import type { ConfigOrigin, PackId } from '../../../lib/ipc/bindings';
import { useConfigIndexEvents, useConfigSearch } from './api';

export interface ConfigSearchFieldProps {
  packId: PackId;
  origin: ConfigOrigin;
  value: string;
  onChange: (value: string) => void;
  /** O campo recebeu o foco (os resultados voltam a aparecer). */
  onFocus: () => void;
  onClear: () => void;
}

export function ConfigSearchField({
  packId,
  origin,
  value,
  onChange,
  onFocus,
  onClear,
}: ConfigSearchFieldProps) {
  const { t } = useTranslation('configsBusca');
  useConfigSearch(packId, origin, '', null);
  useConfigIndexEvents(packId);
  return (
    <div className="cfg-filter">
      <Search className="icon icon--sm" aria-hidden="true" />
      <input
        className="input input--sm"
        type="search"
        value={value}
        placeholder={t('campo')}
        aria-label={t('campo')}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        onFocus={onFocus}
        onKeyDown={(event) => {
          if (event.key === 'Escape' && value !== '') {
            event.preventDefault();
            onClear();
          }
        }}
      />
      {value !== '' ? (
        <Button size="sm" variant="ghost" iconOnly icon={X} onClick={onClear}>
          {t('limpar')}
        </Button>
      ) : null}
    </div>
  );
}
