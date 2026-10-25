/**
 * Barra dos marcados (esboço `design/bug-01/adicionar.html`): a contagem, os nomes, as
 * dependências obrigatórias que já dá para saber (pelo plano da seleção, pedido quando ela para
 * de mudar) e **Adicionar N ao pack →**, que abre o diálogo Adicionar itens.
 */
import { ArrowRight } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import type { AddChoice, PackId, SearchResult } from '../../../lib/ipc/bindings';
import { useKnownDependencies } from '../common/api';
import { joinNames } from '../dependencies/model';
import { preferredRef } from '../modrinth/source';

/** Espera a seleção parar de mudar antes de conferir as dependências. */
const SETTLE_MS = 600;

export function choicesOf(items: readonly SearchResult[]): AddChoice[] {
  return items.flatMap((result) => {
    const ref = preferredRef(result.sources);
    return ref ? [{ source: ref.source, projectId: ref.projectId, versionId: null }] : [];
  });
}

export function SelectionBar({
  packId,
  selected,
  onClear,
  onAdd,
}: {
  packId: PackId;
  selected: readonly SearchResult[];
  onClear: () => void;
  onAdd: () => void;
}) {
  const { t } = useTranslation('adicionar');
  const choices = useMemo(() => choicesOf(selected), [selected]);
  const [settled, setSettled] = useState<AddChoice[]>([]);
  useEffect(() => {
    const timer = setTimeout(() => {
      setSettled(choices);
    }, SETTLE_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [choices]);
  const known = useKnownDependencies(packId, settled);
  const required = (known.data?.nodes ?? [])
    .filter((node) => node.role === 'required')
    .map((node) => node.title);
  const count = selected.length;
  return (
    <div className="selbar addsel" role="region" aria-label={t('selecao.rotulo')}>
      <span className="addsel__n" aria-hidden="true">
        {count}
      </span>
      <div className="addsel__txt">
        <span className="addsel__t" aria-live="polite">
          {t('selecao.contagem', { count })}{' '}
          <span>· {joinNames(selected.map((result) => result.title))}</span>
        </span>
        <span className="addsel__d">
          {required.length > 0 ? (
            <Trans
              t={t}
              i18nKey="selecao.conhecidas"
              count={required.length}
              values={{ names: joinNames(required) }}
              components={{ b: <b /> }}
            />
          ) : (
            t('selecao.proximoPasso')
          )}
        </span>
      </div>
      <Button size="sm" variant="ghost" onClick={onClear}>
        {t('selecao.limpar')}
      </Button>
      <Button variant="primary" iconEnd={ArrowRight} onClick={onAdd}>
        {t('selecao.adicionar', { count })}
      </Button>
    </div>
  );
}
