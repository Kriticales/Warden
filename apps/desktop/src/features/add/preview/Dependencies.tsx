/**
 * Dependências da versão escolhida (SPEC T08, P1; protótipo `pv-dep`): obrigatórias, opcionais e
 * incompatíveis, cada uma com "Já no pack" ou "Será adicionada". O plano de verdade (com a
 * resolução recursiva) é o diálogo de dependências da T09; aqui é só a leitura do que a versão
 * declara.
 */
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Icon } from '../../../components/ui/icon';
import type { DependencyInfo, DependencyKind, PackId, SourceId } from '../../../lib/ipc/bindings';
import { useVersionDependencies } from './api';

const KINDS: readonly DependencyKind[] = ['required', 'optional', 'incompatible'];
const KIND_LABEL = {
  required: 'dependencias.obrigatoria',
  optional: 'dependencias.opcional',
  incompatible: 'dependencias.incompativel',
} as const satisfies Record<DependencyKind, string>;

export interface DependenciesProps {
  packId: PackId;
  source: SourceId;
  versionId: string | null;
}

export function Dependencies({ packId, source, versionId }: DependenciesProps) {
  const { t } = useTranslation('descoberta');
  const dependencies = useVersionDependencies(packId, source, versionId);
  if (versionId === null) return <p className="t-sm t-3">{t('dependencias.escolhaUmaVersao')}</p>;
  if (dependencies.isPending) return <LoadingState inline label={t('dependencias.carregando')} />;
  if (dependencies.isError) {
    return (
      <ErrorPanel
        compact
        title={t('dependencias.erro')}
        error={dependencies.error}
        onRetry={() => {
          void dependencies.refetch();
        }}
      />
    );
  }
  const items = dependencies.data.items;
  return (
    <dl className="kv">
      {KINDS.map((kind) => {
        const group = items.filter((item) => item.kind === kind);
        return (
          <div key={kind} className="kv__row">
            <dt>{t(KIND_LABEL[kind])}</dt>
            <dd>
              {group.length === 0 ? (
                <span className="t-3">{t('dependencias.nenhuma')}</span>
              ) : (
                <ul className="stack-2">
                  {group.map((item) => (
                    <li key={`${item.source}:${item.projectId}`}>
                      <Dependency item={item} />
                    </li>
                  ))}
                </ul>
              )}
            </dd>
          </div>
        );
      })}
    </dl>
  );
}

function Dependency({ item }: { item: DependencyInfo }) {
  const { t } = useTranslation('descoberta');
  return (
    <span className="row row--wrap">
      <span>{item.title}</span>
      {item.kind === 'incompatible' ? (
        item.inPack ? (
          <span className="tag tag--danger">{t('dependencias.conflito')}</span>
        ) : null
      ) : item.inPack ? (
        <span className="tag tag--primary deps__installed">
          <Icon icon={Check} />
          {t('dependencias.jaNoPack')}
        </span>
      ) : item.kind === 'required' ? (
        <span className="tag tag--warn">{t('dependencias.seraAdicionada')}</span>
      ) : (
        <span className="t-xs t-3">{t('dependencias.opcionalNaoAdicionada')}</span>
      )}
    </span>
  );
}
