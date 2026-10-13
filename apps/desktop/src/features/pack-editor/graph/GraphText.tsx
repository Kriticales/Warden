/**
 * Ao lado do grafo focado, em texto (SPEC T06; protótipo `graphContent`): "Em palavras" (o que o
 * mod no centro precisa, quem precisa dele, "Se você remover…", "Por que está no pack"), a
 * lista de todas as ligações com o tipo de cada uma (o equivalente do desenho para o leitor de
 * tela) e as bibliotecas sem uso. "Se você remover" e "Por que está no pack" vêm das mesmas
 * consultas do bloco de dependências dos detalhes (D-07).
 */
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { FocusEdge, FocusReport, Orphan, PackId } from '../../../lib/ipc/bindings';
import { useDependents, useWhyInPack } from '../details/dependencies/api';
import { whyLine } from '../details/dependencies/DependenciesBlock';
import { describeWhy } from '../details/dependencies/why';
import { labels, summarizeCenter } from './model';

const list = (names: readonly string[]) =>
  new Intl.ListFormat('pt-BR', { type: 'conjunction' }).format(names);

export function InWords({ packId, report }: { packId: PackId; report: FocusReport }) {
  const { t } = useTranslation('grafoDesenho');
  const { t: tGrafo } = useTranslation('grafo');
  const center = report.center;
  const path = center?.path ?? '';
  const dependents = useDependents(packId, path, center !== null);
  const why = useWhyInPack(packId, path, center !== null);
  if (!center) return null;
  const name = center.name;
  const summary = summarizeCenter(report);
  const centerNode = report.nodes.find((node) => node.key === center.path);
  const names = (nodes: { label: string; item: unknown }[]) =>
    nodes.length === 0
      ? t('palavras.nenhum')
      : list(
          nodes.map((node) => (node.item ? node.label : `${node.label} (${t('ligacoes.falta')})`)),
        );
  const affected = dependents.data?.affected ?? null;
  return (
    <section className="panel" aria-labelledby="grafo-palavras">
      <h2 className="panel__title panel__title--sans" id="grafo-palavras">
        {t('palavras.titulo')}
      </h2>
      <ul className="stack-2 t-sm grafo-palavras">
        <li>
          <b>{t('palavras.precisaDe', { nome: name })}</b> {names(summary.needs)}
        </li>
        {summary.optional.length > 0 ? (
          <li>
            <b>{t('palavras.usaSeTiver')}</b> {names(summary.optional)}
          </li>
        ) : null}
        {summary.incompatible.length > 0 ? (
          <li>
            <b>{t('palavras.incompativelCom')}</b> {names(summary.incompatible)}
          </li>
        ) : null}
        {centerNode && centerNode.embedded.length > 0 ? (
          <li>
            <b>{t('palavras.dentro', { nome: name })}</b> {list(centerNode.embedded)}
          </li>
        ) : null}
        <li>
          <b>{t('palavras.precisamDele', { nome: name })}</b> {names(summary.neededBy)}
        </li>
        {affected ? (
          <li>
            {affected.length === 0
              ? t('palavras.seRemoverNada', { nome: name })
              : t('palavras.seRemover', {
                  nome: name,
                  count: affected.length,
                  nomes: list(affected.map((entry) => entry.item.name)),
                })}
          </li>
        ) : null}
        {why.data ? (
          <li>
            <b>{t('palavras.porQue')}</b>{' '}
            {describeWhy(why.data)
              .map((line) => whyLine(line, tGrafo))
              .join('. ')}
          </li>
        ) : null}
      </ul>
    </section>
  );
}

/** Todas as ligações do desenho, uma por linha, com o tipo (CA-T06-07: "na lista em texto"). */
export function EdgeList({ report }: { report: FocusReport }) {
  const { t } = useTranslation('grafoDesenho');
  const { t: tGrafo } = useTranslation('grafo');
  const names = labels(report);
  const missing = new Set(report.nodes.filter((node) => !node.item).map((node) => node.key));
  return (
    <section className="panel" aria-labelledby="grafo-ligacoes">
      <h2 className="panel__title panel__title--sans" id="grafo-ligacoes">
        {t('ligacoes.titulo', { count: report.edges.length })}
      </h2>
      {report.edges.length === 0 ? (
        <p className="t-sm t-3">{t('ligacoes.nenhuma')}</p>
      ) : (
        <ul className="stack-2 t-sm grafo-ligacoes">
          {report.edges.map((edge) => (
            <EdgeLine
              key={`${edge.from}|${edge.to}|${edge.kind}`}
              edge={edge}
              from={names.get(edge.from) ?? edge.from}
              to={names.get(edge.to) ?? edge.to}
              missing={missing.has(edge.to)}
              t={t}
              kindText={tGrafo(`tipo.${edge.kind}`)}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

function EdgeLine({
  edge,
  from,
  to,
  missing,
  t,
  kindText,
}: {
  edge: FocusEdge;
  from: string;
  to: string;
  missing: boolean;
  t: ReturnType<typeof useTranslation<'grafoDesenho'>>['t'];
  kindText: string;
}) {
  const target = edge.embedded ?? to;
  return (
    <li>
      {t(`ligacoes.${edge.kind}`, { de: from, para: target })}
      <span className="t-3"> · {kindText}</span>
      {edge.embedded ? (
        <span className="t-3"> · {t('ligacoes.dentroDe', { nome: to })}</span>
      ) : null}
      {missing ? <span className="t-danger"> · {t('ligacoes.falta')}</span> : null}
      {edge.range ? <span className="t-3 t-mono"> · {edge.range}</span> : null}
      {edge.note ? <div className="t-xs t-3">{edge.note}</div> : null}
    </li>
  );
}

/** "1 biblioteca sem uso: Kotlin for Forge." com "Ver …" para abrir os detalhes. */
export function OrphansNote({
  orphans,
  onOpen,
}: {
  orphans: readonly Orphan[];
  onOpen: (path: string) => void;
}) {
  const { t } = useTranslation('grafoDesenho');
  if (orphans.length === 0) return null;
  const first = orphans[0];
  return (
    <Alert
      kind="info"
      compact
      title={t('orfas.titulo', {
        count: orphans.length,
        nomes: list(orphans.map((orphan) => orphan.item.name)),
      })}
      actions={
        first ? (
          <Button
            size="sm"
            onClick={() => {
              onOpen(first.item.path);
            }}
          >
            {t('orfas.ver', { nome: first.item.name })}
          </Button>
        ) : null
      }
    >
      <div className="alert__text">{t('orfas.texto')}</div>
    </Alert>
  );
}
