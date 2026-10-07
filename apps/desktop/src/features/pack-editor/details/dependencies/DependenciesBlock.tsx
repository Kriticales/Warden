/**
 * "Depende de / Usado por / Por que está no pack" nos detalhes do item (SPEC T07; CA-T07-03) e
 * "Se você remover…" (T06; CA-T06-06). Tudo vem das consultas do grafo (`graph_*`, D-07), que
 * já consideram `provides` e jar-in-jar; aqui só se mostra.
 */
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { LoadingState } from '../../../../components/common/LoadingState';
import { formatDate } from '../../../../lib/format';
import type {
  DependentsReport,
  DependsOnState,
  RelationKind,
  WhyReport,
} from '../../../../lib/ipc/bindings';
import type { DetailBlockProps } from '../blocks';
import { useDependents, useWhyInPack } from './api';
import { describeWhy, type WhyLine } from './why';

export function DependenciesBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('grafo');
  // Só mods têm dependências de mod; arquivo inválido não entra no grafo.
  const applies = item.kind === 'mod' && item.state === 'ok';
  const dependents = useDependents(packId, item.path, applies);
  const why = useWhyInPack(packId, item.path, applies);
  if (!applies) {
    return null;
  }
  const failure = dependents.error ?? why.error;
  if (failure) {
    return (
      <section className="stack-2" aria-label={t('titulo')}>
        <ErrorPanel
          compact
          error={failure}
          onRetry={() => {
            void dependents.refetch();
            void why.refetch();
          }}
        />
      </section>
    );
  }
  if (!dependents.data || !why.data) {
    return <LoadingState inline label={t('carregando')} />;
  }
  return (
    <section aria-label={t('titulo')}>
      <dl className="kv">
        <dt>{t('dependeDe')}</dt>
        <dd>
          <DependsOnList report={dependents.data} />
        </dd>
        <dt>{t('usadoPor')}</dt>
        <dd>
          <UsedByList report={dependents.data} />
        </dd>
        <dt>{t('porQueEstaNoPack')}</dt>
        <dd>
          <WhyText report={why.data} />
        </dd>
        {dependents.data.affected.length > 0 ? (
          <>
            <dt>{t('seRemover')}</dt>
            <dd>
              <Affected report={dependents.data} />
            </dd>
          </>
        ) : null}
      </dl>
    </section>
  );
}

type StateKey = 'inPack' | 'own' | 'missing' | 'breaksInPack' | 'breaksMissing';

function isIncompatible(kind: RelationKind): boolean {
  return kind === 'breaks' || kind === 'conflicts';
}

/** Incompatibilidade tem textos próprios: "está no pack" é o problema, "não está" é o normal. */
function stateKey(kind: RelationKind, state: DependsOnState): StateKey {
  if (!isIncompatible(kind)) return state;
  return state === 'missing' ? 'breaksMissing' : 'breaksInPack';
}

function DependsOnList({ report }: { report: DependentsReport }) {
  const { t } = useTranslation('grafo');
  if (report.dependsOn.length === 0) {
    return <span className="t-3">{t('nenhumModDependeDe')}</span>;
  }
  return (
    <ul className="stack-2">
      {report.dependsOn.map((relation) => {
        const incompatible = isIncompatible(relation.kind);
        const state = stateKey(relation.kind, relation.state);
        const provider = relation.providers[0];
        // Em vermelho: dependência que falta ou incompatível que está no pack.
        const bad = incompatible ? relation.state === 'inPack' : relation.state === 'missing';
        const key = `${relation.kind}:${relation.id}:${relation.range ?? ''}`;
        return (
          <li key={key}>
            <span className="t-mono">{relation.id}</span>{' '}
            <span className="t-3">· {t(`tipo.${relation.kind}`)}</span>
            {relation.range ? <span className="t-3"> · {relation.range}</span> : null}{' '}
            <span className={bad ? 't-danger' : 't-3'}>· {t(`estado.${state}`)}</span>
            {provider && relation.state === 'inPack' && !incompatible ? (
              <span className="t-3">
                {' '}
                ·{' '}
                {provider.embedded
                  ? t('fornecidoPorEmbutido', { nome: provider.item.name })
                  : t('fornecidoPor', { nome: provider.item.name })}
                {provider.alias ? ` ${t('fornecidoComoAlias', { id: provider.modId })}` : ''}
              </span>
            ) : null}
            {relation.note ? <div className="t-xs t-3">{relation.note}</div> : null}
          </li>
        );
      })}
    </ul>
  );
}

function UsedByList({ report }: { report: DependentsReport }) {
  const { t } = useTranslation('grafo');
  if (report.usedBy.length === 0) {
    return <span className="t-3">{t('nenhumModUsa')}</span>;
  }
  return (
    <ul className="stack-2">
      {report.usedBy.map((use) => (
        <li key={`${use.item.path}:${use.kind}:${use.id}`}>
          <span>{use.item.name}</span> <span className="t-3">· {t(`tipo.${use.kind}`)}</span>
          {use.embedded ? <span className="t-3"> · {t('dentroDesteMod')}</span> : null}
          {use.note ? <div className="t-xs t-3">{use.note}</div> : null}
        </li>
      ))}
    </ul>
  );
}

function WhyText({ report }: { report: WhyReport }) {
  const { t } = useTranslation('grafo');
  const lines = describeWhy(report);
  return (
    <div className="stack-2">
      {lines.map((line) => (
        <p key={line.key}>{whyLine(line, t)}</p>
      ))}
    </div>
  );
}

function whyLine(line: WhyLine, t: TFunction<'grafo'>) {
  switch (line.kind) {
    case 'user':
      return line.at === null
        ? t('porque.voce')
        : t('porque.voceEm', { data: formatDate(line.at) });
    case 'noDependents':
      return t('porque.ninguemExige');
    case 'chain':
      return [
        t('porque.exigidoPor', { nome: line.names[0] ?? '' }),
        ...line.names.slice(1).map((nome) => t('porque.queEExigidoPor', { nome })),
        t('porque.queVoceAdicionou'),
      ].join(', ');
    case 'unused':
      return line.optionalUsers.length === 0
        ? t('porque.semUso')
        : `${t('porque.semUso')}. ${t('porque.soOpcional', {
            count: line.optionalUsers.length,
            nomes: line.optionalUsers.join(', '),
          })}`;
    case 'cycle':
      return t('porque.ciclo', { nomes: line.names.join(', ') });
  }
}

function Affected({ report }: { report: DependentsReport }) {
  const { t } = useTranslation('grafo');
  return (
    <span>
      {t('afetados', {
        count: report.affected.length,
        nomes: report.affected.map((affected) => affected.item.name).join(', '),
      })}
    </span>
  );
}
