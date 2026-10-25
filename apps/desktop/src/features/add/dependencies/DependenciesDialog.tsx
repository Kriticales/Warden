/**
 * Diálogo "Adicionar itens" (SPEC T09; esboço aprovado `design/bug-01/adicionar-itens.html`,
 * opção 4 · Árvore + detalhe; ADR-0068): aparece antes de gravar qualquer coisa, para um item ou
 * vários de uma vez (seleção, kits, modpacks, links).
 *
 * À esquerda, a árvore: um bloco por mod escolhido, com o que ele traz pendurado (desmarcar o mod
 * apaga o galho), "Em comum" uma vez só e o que fica de fora apagado na própria árvore, com selo.
 * No topo dela, o que pede decisão ("2 itens precisam de você") e o aviso de lado não informado,
 * uma vez. À direita, o porquê do item clicado: a conta em blocos no alto e, embaixo, situação,
 * versão, arquivo, fonte, lado, o que faz e quem pede, com o botão que resolve o problema dele.
 *
 * A conta é a mesma no título, no botão e no rodapé ("3 escolhidos + 2 obrigatórias + 2
 * opcionais = 7 itens"); a regra do que entra é a de `model.ts`. O botão grava tudo de uma vez:
 * ou todos entram, ou nenhum.
 */
import {
  Ban,
  Check,
  CircleCheck,
  Copy,
  FlaskConical,
  Info,
  OctagonAlert,
  Plus,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';
import { type CSSProperties, Fragment, type ReactNode, useMemo, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { cn } from '../../../lib/cn';
import type { AddChoice, AddPlan, AddResult, PackId, PlanNode } from '../../../lib/ipc/bindings';
import { usePack } from '../../packs/api';
import { useAddApply, useAddPlan } from '../common/api';
import { ProjectIcon } from '../common/ProjectIcon';
import { type PackTarget, SourceDots, SourceLegend, useTargetShort } from '../common/text';
import {
  activeConflicts,
  applyRequest,
  type Attention,
  attentionItems,
  byAllChosen,
  type DependencyChoices,
  firstCurrent,
  includedKeys,
  joinNames,
  NO_CHOICES,
  onlyIncluded,
  planTree,
  type RoleCounts,
  roleCounts,
  type RowState,
  rowState,
  type RowTag,
  setIncluded,
  titlesOf,
  type TreeItem,
  type TreeRow,
  unknownSide,
  withKnownTitles,
} from './model';
import './dependencies.css';

/** O que abre o diálogo: os itens escolhidos e os nomes deles (para o título). */
export interface DependenciesRequest {
  choices: AddChoice[];
  titles: string[];
}

export interface DependenciesDialogProps {
  packId: PackId;
  request: DependenciesRequest | null;
  target: PackTarget;
  onClose: () => void;
  onAdded: (result: AddResult) => void;
}

export function DependenciesDialog({
  packId,
  request,
  target,
  onClose,
  onAdded,
}: DependenciesDialogProps) {
  const { t } = useTranslation('adicionar');
  const pack = usePack(packId);
  const alvo = useTargetShort(target);
  const plan = useAddPlan(packId, request?.choices ?? null);
  // O plano com os nomes que a busca já mostrou (um item sem versão pode vir só com o ID).
  const data = useMemo(
    () =>
      plan.data && request ? withKnownTitles(plan.data, request.choices, request.titles) : null,
    [plan.data, request],
  );
  const apply = useAddApply(packId);
  const [choices, setChoices] = useState<DependencyChoices>(NO_CHOICES);
  const [current, setCurrent] = useState<string | null>(null);
  const included = useMemo(
    () => (data ? includedKeys(data, choices) : new Set<string>()),
    [data, choices],
  );
  const counts = data ? roleCounts(data, included) : null;

  // Antes do plano, os nomes pedidos; depois, a mesma conta do botão e do rodapé.
  const title =
    request === null
      ? ''
      : counts
        ? counts.total === 0
          ? t('dependencias.tituloNada')
          : t('dependencias.titulo', { count: counts.total })
        : request.titles.length === 1
          ? t('dependencias.tituloUm', { name: request.titles[0] ?? '' })
          : t('dependencias.tituloVarios', { count: request.titles.length });
  const packName = pack.data?.name;

  const close = () => {
    if (apply.isPending) return;
    setChoices(NO_CHOICES);
    setCurrent(null);
    apply.reset();
    onClose();
  };

  const confirm = (data: AddPlan) => {
    apply.mutate(applyRequest(data, included, choices), {
      onSuccess: (result) => {
        if (result.added.length > 0) {
          showToast({
            kind: 'ok',
            title: t('dependencias.feito', { count: result.added.length }),
            text: `${joinNames(result.added.map((item) => item.title))}.`,
          });
        } else {
          showToast({
            kind: 'info',
            title: t('dependencias.jaEstavam', { count: result.skipped.length }),
          });
        }
        setChoices(NO_CHOICES);
        setCurrent(null);
        apply.reset();
        onAdded(result);
      },
    });
  };

  return (
    <Dialog
      open={request !== null}
      onOpenChange={(open) => {
        if (!open) close();
      }}
    >
      {request !== null ? (
        <DialogContent
          size="xl"
          title={title}
          description={
            packName
              ? t('dependencias.sub', { pack: packName, alvo })
              : t('dependencias.subSemPack', { alvo })
          }
          aside={<SourceLegend />}
          footer={
            <>
              {counts ? <Sum counts={counts} /> : <span className="grow" />}
              <Button variant="ghost" onClick={close} disabled={apply.isPending}>
                {t('acoes.cancelar', { ns: 'comum' })}
              </Button>
              <Button
                variant="primary"
                icon={Plus}
                loading={apply.isPending}
                disabled={!data || included.size === 0}
                onClick={() => {
                  if (data) confirm(data);
                }}
              >
                {apply.isPending
                  ? t('dependencias.adicionando')
                  : t('dependencias.adicionar', { count: included.size })}
              </Button>
            </>
          }
        >
          {plan.isError ? (
            <ErrorPanel
              title={t('dependencias.erro')}
              error={plan.error}
              onRetry={() => {
                void plan.refetch();
              }}
            />
          ) : data === null ? (
            <LoadingState label={t('dependencias.carregando')} />
          ) : (
            <PlanView
              plan={data}
              alvo={alvo}
              included={included}
              choices={choices}
              current={current}
              onCurrent={setCurrent}
              onChange={setChoices}
            />
          )}
          {apply.isError ? (
            <ErrorPanel compact title={t('dependencias.erroGravar')} error={apply.error} />
          ) : null}
        </DialogContent>
      ) : null}
    </Dialog>
  );
}

/** "3 escolhidos + 2 obrigatórias + 2 opcionais = 7 itens" (a conta do botão). */
function Sum({ counts }: { counts: RoleCounts }) {
  const { t } = useTranslation('adicionar');
  if (counts.total === 0) {
    return <p className="conta conta--none">{t('dependencias.conta.nada')}</p>;
  }
  const parts: string[] = [t('dependencias.conta.escolhidos', { count: counts.chosen })];
  if (counts.required > 0) {
    parts.push(t('dependencias.conta.obrigatorias', { count: counts.required }));
  }
  if (counts.optional > 0) {
    parts.push(t('dependencias.conta.opcionais', { count: counts.optional }));
  }
  return (
    <p className="conta">
      {parts.map((part, index) => (
        <Fragment key={part}>
          {index > 0 ? <span className="conta__op"> + </span> : null}
          <span>{part}</span>
        </Fragment>
      ))}
      <span className="conta__op"> = </span>
      <span className="conta__tot">{t('dependencias.conta.itens', { count: counts.total })}</span>
    </p>
  );
}

interface PlanViewProps {
  plan: AddPlan;
  alvo: string;
  included: ReadonlySet<string>;
  choices: DependencyChoices;
  current: string | null;
  onCurrent: (key: string) => void;
  onChange: (choices: DependencyChoices) => void;
}

function PlanView({ plan, alvo, included, choices, current, onCurrent, onChange }: PlanViewProps) {
  const { t } = useTranslation('adicionar');
  const tree = useMemo(() => planTree(plan), [plan]);
  const attention = attentionItems(plan, included, choices);
  const unknown = unknownSide(plan, included);
  const shown = current ?? firstCurrent(plan, attention);
  const toggle = (key: string, on: boolean) => {
    onChange(setIncluded(plan, choices, key, on));
  };
  const rows = (branch: readonly TreeRow[], common: boolean) =>
    branch.map((row, index) => (
      <Row
        key={row.item.key}
        plan={plan}
        row={row}
        top={index === 0}
        common={common}
        alvo={alvo}
        included={included}
        choices={choices}
        current={shown === row.item.key}
        onCurrent={onCurrent}
        onToggle={toggle}
      />
    ));
  return (
    <div className="deps4">
      <div className="deps4__tree">
        {attention.length > 0 || unknown.length > 0 ? (
          <div className="deps4__top">
            {attention.length > 0 ? (
              <AttentionLine items={attention} onCurrent={onCurrent} />
            ) : null}
            {unknown.length > 0 ? (
              <p className="deps-note">
                <Icon icon={Info} size="sm" />
                <span>
                  <Trans
                    t={t}
                    i18nKey="dependencias.lado"
                    count={unknown.length}
                    values={{ names: joinNames(unknown.map((node) => node.title)) }}
                    components={{ b: <b /> }}
                  />
                </span>
              </p>
            ) : null}
          </div>
        ) : null}
        <div className="dtree" role="table" aria-label={t('dependencias.arvore')}>
          <div className="dtree__row dtree__head" role="row">
            <span role="columnheader">{t('dependencias.colunas.mod')}</span>
            <span role="columnheader">{t('dependencias.colunas.versao')}</span>
            <span role="columnheader">{t('dependencias.colunas.papel')}</span>
            <span role="columnheader">{t('dependencias.colunas.situacao')}</span>
          </div>
          {tree.branches.map((branch) => {
            const root = branch[0]?.item;
            const off = root?.kind === 'node' && !included.has(root.key);
            return (
              <div
                key={root?.key ?? 'galho'}
                className={cn('dtree__branch', off && 'is-off')}
                role="rowgroup"
              >
                {rows(branch, false)}
              </div>
            );
          })}
          {tree.common.length > 0 ? (
            <div className="dtree__branch dtree__branch--common" role="rowgroup">
              <div role="row">
                <div className="dtree__common" role="cell">
                  <h3>{t('dependencias.emComum')}</h3>
                  <span>{t('dependencias.emComumDica')}</span>
                </div>
              </div>
              {tree.common.map((branch) => rows(branch, true))}
            </div>
          ) : null}
        </div>
        {plan.conflicts.length === 0 ? (
          <p className="deps-note">
            <Icon icon={CircleCheck} size="sm" className="t-ok" />
            <span>{t('dependencias.semIncompatibilidade', { count: plan.packItemCount })}</span>
          </p>
        ) : null}
      </div>
      <aside className="deps4__side" aria-label={t('dependencias.detalhe.rotulo')}>
        <Blocks plan={plan} included={included} current={shown} onCurrent={onCurrent} />
        {shown !== null ? (
          <Detail
            key={shown}
            plan={plan}
            itemKey={shown}
            alvo={alvo}
            included={included}
            choices={choices}
            onChange={onChange}
          />
        ) : null}
      </aside>
    </div>
  );
}

/** "2 itens precisam de você antes de adicionar": Embeddium incompatível com Sodium · … */
function AttentionLine({
  items,
  onCurrent,
}: {
  items: readonly Attention[];
  onCurrent: (key: string) => void;
}) {
  const { t } = useTranslation('adicionar');
  const danger = items.some((item) => item.severity === 'danger');
  const why = (item: Attention) => {
    switch (item.reason) {
      case 'incompativel':
        return t('dependencias.atencaoMotivo.incompativel', { other: item.other ?? '' });
      case 'duplicado':
        return t('dependencias.atencaoMotivo.duplicado', {
          pela: t(`dependencias.pela.${item.source ?? 'link'}`),
        });
      case 'obrigatoria':
        return t('dependencias.atencaoMotivo.obrigatoria');
    }
  };
  return (
    <div className={cn('pb', danger ? 'pb--danger' : 'pb--warn')} role="status">
      <Icon icon={danger ? OctagonAlert : TriangleAlert} />
      <span className="pb__t">{t('dependencias.atencao', { count: items.length })}</span>
      <span className="pb__d pb__list">
        {items.map((item, index) => (
          <Fragment key={`${item.key}:${item.reason}`}>
            {index > 0 ? (
              <span className="pb__sep" aria-hidden="true">
                ·
              </span>
            ) : null}
            <span>
              <Button
                variant="link"
                onClick={() => {
                  onCurrent(item.key);
                }}
              >
                {item.title}
              </Button>{' '}
              {why(item)}
            </span>
          </Fragment>
        ))}
      </span>
    </div>
  );
}

const TAG_KIND: Record<RowTag, 'warn' | 'danger' | 'off'> = {
  beta: 'warn',
  alpha: 'warn',
  semVersao: 'off',
  incompativel: 'danger',
  jaNoPack: 'warn',
  substitui: 'warn',
  desmarcado: 'off',
  naoEntra: 'off',
  jogoPodeNaoAbrir: 'danger',
  versaoIncompativel: 'warn',
};

const TAG_ICON: Partial<Record<RowTag, LucideIcon>> = {
  beta: FlaskConical,
  alpha: FlaskConical,
  incompativel: OctagonAlert,
  jaNoPack: Copy,
  substitui: Copy,
  jogoPodeNaoAbrir: OctagonAlert,
  versaoIncompativel: TriangleAlert,
};

function useTagText() {
  const { t } = useTranslation('adicionar');
  return (tag: RowTag, plan: AddPlan, key: string, alvo: string): [string, string] => {
    const name = titlesOf(plan, [key])[0] ?? key;
    switch (tag) {
      case 'beta':
      case 'alpha':
        return [
          t(`dependencias.tag.${tag}`),
          t('dependencias.tag.foraDoCanalDica', {
            alvo,
            canal: t(`dependencias.canal.${tag}`).toLowerCase(),
          }),
        ];
      case 'semVersao':
        return [
          t('dependencias.tag.semVersao'),
          t('dependencias.tag.semVersaoDica', { name, alvo }),
        ];
      case 'incompativel': {
        const conflict = plan.conflicts.find((item) => item.item.key === key);
        return [
          t('dependencias.tag.incompativel'),
          t('dependencias.tag.incompativelDica', { other: conflict?.other.title ?? '' }),
        ];
      }
      case 'jaNoPack': {
        const duplicate = plan.duplicates.find((item) => item.key === key);
        return [
          t('dependencias.tag.jaNoPack'),
          t('dependencias.tag.jaNoPackDica', {
            pela: t(`dependencias.pela.${duplicate?.existingSource ?? 'link'}`),
            path: duplicate?.existingPath ?? '',
          }),
        ];
      }
      case 'substitui':
        return [t('dependencias.tag.substitui'), t('dependencias.tag.substituiDica', { name })];
      case 'desmarcado':
        return [t('dependencias.tag.desmarcado'), t('dependencias.tag.desmarcadoDica')];
      case 'naoEntra':
        return [t('dependencias.tag.naoEntra'), t('dependencias.tag.naoEntra')];
      case 'jogoPodeNaoAbrir':
        return [t('dependencias.tag.jogoPodeNaoAbrir'), t('dependencias.tag.jogoPodeNaoAbrirDica')];
      case 'versaoIncompativel':
        return [
          t('dependencias.tag.versaoIncompativel'),
          t('dependencias.tag.versaoIncompativelDica', { alvo }),
        ];
    }
  };
}

function RowTagView({ tag, label, hint }: { tag: RowTag; label: string; hint: string }) {
  const kind = TAG_KIND[tag];
  const icon = TAG_ICON[tag];
  return (
    <span className={cn('tag', kind === 'off' ? 'tag--plain t-3' : `tag--${kind}`)} title={hint}>
      {icon ? <Icon icon={icon} /> : null}
      {label}
    </span>
  );
}

interface RowProps {
  plan: AddPlan;
  row: TreeRow;
  /** Primeira linha do bloco (o mod escolhido, ou a linha de "Em comum"). */
  top: boolean;
  common: boolean;
  alvo: string;
  included: ReadonlySet<string>;
  choices: DependencyChoices;
  current: boolean;
  onCurrent: (key: string) => void;
  onToggle: (key: string, on: boolean) => void;
}

function Row({
  plan,
  row,
  top,
  common,
  alvo,
  included,
  choices,
  current,
  onCurrent,
  onToggle,
}: RowProps) {
  const { t } = useTranslation('adicionar');
  const tagText = useTagText();
  const { item } = row;
  const head = top && !common;
  const title = titlesOf(plan, [item.key])[0] ?? item.key;
  const state: RowState | null =
    item.kind === 'node' ? rowState(plan, item.node, included, choices) : null;
  const tag: RowTag | null = item.kind === 'missing' ? 'semVersao' : (state?.tag ?? null);
  const off = item.kind === 'missing' || (state !== null && !state.checked);
  const source = item.kind === 'node' ? item.node.source : null;
  const role =
    item.kind === 'node' ? item.node.role : item.kind === 'missing' ? item.item.role : null;
  return (
    <div
      className={cn(
        'dtree__row',
        head && 'dtree__row--head',
        common && top && 'dtree__row--flat',
        off && 'is-off',
        current && 'is-current',
      )}
      style={{ '--depth': row.depth } as CSSProperties}
      role="row"
      data-last={row.last || undefined}
    >
      <span className="dtree__lead" role="cell">
        {item.kind === 'installed' ? (
          <span className="dtree__in" title={t('dependencias.jaNoPackIcone')}>
            <Icon icon={CircleCheck} size="sm" />
          </span>
        ) : (
          <label className="check dtree__check">
            <input
              type="checkbox"
              checked={state?.checked ?? false}
              disabled={item.kind === 'missing' || (state?.disabled ?? true)}
              aria-label={t('dependencias.marcarItem', { name: title })}
              onChange={(event) => {
                onToggle(item.key, event.target.checked);
              }}
            />
          </label>
        )}
        <ProjectIcon
          url={item.kind === 'node' ? item.node.iconUrl : null}
          name={title}
          className="dtree__icon"
          size="sm"
        />
        <button
          type="button"
          className="dtree__name"
          aria-current={current ? 'true' : undefined}
          title={t('dependencias.abrirItem', { name: title })}
          onClick={() => {
            onCurrent(item.key);
          }}
        >
          {title}
        </button>
      </span>
      <span
        className="dtree__ver"
        role="cell"
        title={item.kind === 'node' ? item.node.fileName : undefined}
      >
        {item.kind === 'node' ? item.node.versionNumber : '—'}
      </span>
      <span role="cell">
        {head && source ? (
          <SourceDots sources={[{ source }]} label />
        ) : item.kind === 'installed' ? (
          <span className="tag tag--ok">
            <Icon icon={Check} />
            {t('dependencias.tag.noPack')}
          </span>
        ) : role === 'required' || role === 'optional' ? (
          <span className={cn('tag tag--plain', role === 'optional' && 't-3')}>
            {t(`dependencias.papel.${role}`)}
          </span>
        ) : source ? (
          <SourceDots sources={[{ source }]} label />
        ) : null}
      </span>
      <span role="cell">
        {tag ? <RowTagView tag={tag} {...tagLabels(tagText(tag, plan, item.key, alvo))} /> : null}
      </span>
      {common && top ? (
        <span className="dtree__rel" role="cell">
          <Relation plan={plan} item={item} included={included} />
        </span>
      ) : null}
    </div>
  );
}

function tagLabels([label, hint]: [string, string]) {
  return { label, hint };
}

/** Os blocos da conta ("7 itens vão entrar no pack": ■■■ + ■■ + ■■). */
function Blocks({
  plan,
  included,
  current,
  onCurrent,
}: {
  plan: AddPlan;
  included: ReadonlySet<string>;
  current: string | null;
  onCurrent: (key: string) => void;
}) {
  const { t } = useTranslation('adicionar');
  const counts = roleCounts(plan, included);
  const groups = (['chosen', 'required', 'optional'] as const).filter((role) => counts[role] > 0);
  const label = (role: PlanNode['role']) =>
    role === 'chosen'
      ? t('dependencias.conta.escolhidos', { count: counts.chosen })
      : role === 'required'
        ? t('dependencias.conta.obrigatorias', { count: counts.required })
        : t('dependencias.conta.opcionais', { count: counts.optional });
  return (
    <div className="cv">
      <p className="cv__t">
        {counts.total > 0 ? (
          <Trans
            t={t}
            i18nKey="dependencias.conta.vaoEntrar"
            count={counts.total}
            components={{ b: <b /> }}
          />
        ) : (
          t('dependencias.conta.nadaVaiEntrar')
        )}
      </p>
      {counts.total > 0 ? (
        <div className="cv__bar" role="group" aria-label={t('dependencias.conta.blocos')}>
          {groups.map((role, index) => (
            <Fragment key={role}>
              {index > 0 ? (
                <span className="cv__eq" aria-hidden="true">
                  +
                </span>
              ) : null}
              <div className={`cv__grp cv__grp--${role}`}>
                <div className="cv__blocks">
                  {plan.nodes
                    .filter((node) => node.role === role && included.has(node.key))
                    .map((node) => (
                      <button
                        key={node.key}
                        type="button"
                        title={node.title}
                        aria-label={node.title}
                        aria-current={current === node.key ? 'true' : undefined}
                        onClick={() => {
                          onCurrent(node.key);
                        }}
                      />
                    ))}
                </div>
                <span className="cv__lbl">{label(role)}</span>
              </div>
            </Fragment>
          ))}
        </div>
      ) : null}
    </div>
  );
}

/** "Pedida por A e B", "Opcional para A", "Traz X e Y"… (só cita quem entra). */
function Relation({
  plan,
  item,
  included,
}: {
  plan: AddPlan;
  item: TreeItem;
  included: ReadonlySet<string>;
}) {
  const { t } = useTranslation('adicionar');
  const names = (keys: readonly string[]) => {
    const shown = onlyIncluded(keys, included);
    const list = shown.length > 0 ? shown : keys;
    return byAllChosen(plan, list)
      ? t('dependencias.relacao.osEscolhidos', { count: list.length })
      : joinNames(titlesOf(plan, list));
  };
  const rich = (key: string, values: Record<string, string>) => (
    <Trans t={t} i18nKey={key as never} values={values} components={{ b: <b /> }} />
  );
  if (item.kind === 'installed') {
    return rich('dependencias.relacao.pedidoPor', { names: names(item.item.requiredBy) });
  }
  if (item.kind === 'missing') {
    return item.item.requiredBy.length > 0
      ? rich('dependencias.relacao.pedidaPor', { names: names(item.item.requiredBy) })
      : null;
  }
  const { node } = item;
  if (node.role === 'chosen') {
    const duplicate = plan.duplicates.find((entry) => entry.key === node.key);
    if (duplicate) {
      return (
        <>
          {t('dependencias.relacao.duplicado', {
            pela: t(`dependencias.pela.${duplicate.existingSource ?? 'link'}`),
          })}
        </>
      );
    }
    const brings = plan.nodes
      .filter((other) => other.role === 'required' && other.requiredBy.includes(node.key))
      .map((other) => other.key);
    return brings.length > 0 ? (
      rich('dependencias.relacao.traz', { names: joinNames(titlesOf(plan, brings)) })
    ) : (
      <>{t('dependencias.relacao.semDependencia')}</>
    );
  }
  if (node.role === 'required') {
    return rich('dependencias.relacao.pedidaPor', { names: names(node.requiredBy) });
  }
  return rich('dependencias.relacao.opcionalPara', { names: names(node.optionalFor) });
}

interface DetailProps {
  plan: AddPlan;
  itemKey: string;
  alvo: string;
  included: ReadonlySet<string>;
  choices: DependencyChoices;
  onChange: (choices: DependencyChoices) => void;
}

/** O porquê do item clicado (coluna da direita). */
function Detail({ plan, itemKey, alvo, included, choices, onChange }: DetailProps) {
  const { t } = useTranslation('adicionar');
  const node = plan.nodes.find((item) => item.key === itemKey);
  const installed = plan.installed.find((item) => item.key === itemKey);
  const missing = plan.missing.find((item) => item.key === itemKey);
  const item: TreeItem | null = node
    ? { kind: 'node', key: itemKey, node }
    : installed
      ? { kind: 'installed', key: itemKey, item: installed }
      : missing
        ? { kind: 'missing', key: itemKey, item: missing }
        : null;
  if (item === null) return null;
  const title = titlesOf(plan, [itemKey])[0] ?? itemKey;
  const head = (role: ReactNode) => (
    <div className="d2__head">
      <ProjectIcon url={node?.iconUrl ?? null} name={title} className="d2__icon" size="lg" />
      <h3>{title}</h3>
      <p className="d2__role">{role}</p>
    </div>
  );
  if (item.kind === 'installed') {
    const other = item.item.packSource !== null && !itemKey.startsWith(`${item.item.packSource}:`);
    return (
      <div className="d2">
        {head(
          <>
            {t('dependencias.relacao.jaNoPack')}
            {other ? ` · ${t(`dependencias.pela.${item.item.packSource ?? 'link'}`)}` : ''}
          </>,
        )}
        <dl className="kv d2__kv">
          <dt>{t('dependencias.detalhe.porQue')}</dt>
          <dd>
            <Relation plan={plan} item={item} included={included} />
          </dd>
          <dt>{t('dependencias.detalhe.caminho')}</dt>
          <dd className="t-mono t-sm">{item.item.path}</dd>
        </dl>
      </div>
    );
  }
  if (item.kind === 'missing') {
    const parents = item.item.requiredBy;
    return (
      <div className="d2">
        {head(
          <>
            {t(
              item.item.role === 'chosen'
                ? 'dependencias.papel.chosen'
                : item.item.role === 'required'
                  ? 'dependencias.papel.requiredTitulo'
                  : 'dependencias.papel.optionalTitulo',
            )}
            {parents.length > 0 ? (
              <>
                {' · '}
                <Relation plan={plan} item={item} included={included} />
              </>
            ) : null}
          </>,
        )}
        <div className="pb pb--done">
          <Icon icon={Ban} />
          <span className="pb__t">
            {t('dependencias.problema.semVersao', { name: title, alvo })}
          </span>
          <span className="pb__d">
            {item.item.role === 'chosen'
              ? t('dependencias.problema.semVersaoEscolhido')
              : item.item.role === 'optional'
                ? t('dependencias.problema.semVersaoOpcional', {
                    names: joinNames(titlesOf(plan, parents)),
                  })
                : t('dependencias.problema.semVersaoObrigatoria', {
                    names: joinNames(titlesOf(plan, parents)),
                  })}
          </span>
        </div>
        <dl className="kv d2__kv">
          <dt>{t('dependencias.detalhe.situacao')}</dt>
          <dd className="t-3">
            {t('dependencias.detalhe.ficaDeForaPorque', {
              motivo: t('dependencias.detalhe.motivo.semVersao', { alvo }),
            })}
          </dd>
          <dt>{t('dependencias.detalhe.versao')}</dt>
          <dd className="t-3">{t('dependencias.detalhe.nenhumaVersao', { alvo })}</dd>
        </dl>
      </div>
    );
  }
  const current = item.node;
  const state = rowState(plan, current, included, choices);
  const parents = current.role === 'optional' ? current.optionalFor : current.requiredBy;
  const outText = (() => {
    switch (state.out) {
      case 'semVersao':
        return t('dependencias.detalhe.motivo.semVersao', { alvo });
      case 'opcional':
        return t('dependencias.detalhe.motivo.opcional');
      case 'manteve':
        return t('dependencias.detalhe.motivo.manteve');
      case 'desmarcou':
        return t('dependencias.detalhe.motivo.desmarcou');
      case 'pedidoDesmarcado':
        return t('dependencias.detalhe.motivo.pedidoDesmarcado', {
          count: parents.length,
          names: joinNames(titlesOf(plan, parents)),
        });
      case null:
        return null;
    }
  })();
  const role =
    current.role === 'chosen'
      ? t('dependencias.papel.chosen')
      : current.role === 'required'
        ? t('dependencias.papel.requiredTitulo')
        : t('dependencias.papel.optionalTitulo');
  return (
    <div className="d2">
      {head(
        <>
          {role}
          {' · '}
          <Relation plan={plan} item={item} included={included} />
        </>,
      )}
      <Problems
        plan={plan}
        node={current}
        alvo={alvo}
        included={included}
        choices={choices}
        onChange={onChange}
      />
      <p className="d2__what">{current.summary || t('dependencias.detalhe.semResumo')}</p>
      <dl className="kv d2__kv">
        <dt>{t('dependencias.detalhe.situacao')}</dt>
        <dd>
          {state.checked ? (
            <span className="t-ok">{t('dependencias.detalhe.vaiEntrar')}</span>
          ) : (
            <span className="t-3">
              {outText
                ? t('dependencias.detalhe.ficaDeForaPorque', { motivo: outText })
                : t('dependencias.detalhe.ficaDeFora')}
            </span>
          )}
        </dd>
        <dt>{t('dependencias.detalhe.versao')}</dt>
        <dd>
          <span className="t-mono">{current.versionNumber}</span>
          <span className="d2__sub">
            {current.outsideChannel && current.channel !== 'release'
              ? t('dependencias.detalhe.versaoForaDoCanal', {
                  canal: t(`dependencias.canal.${current.channel}`),
                  alvo,
                })
              : t('dependencias.detalhe.versaoEstavel', { alvo })}
          </span>
        </dd>
        <dt>{t('dependencias.detalhe.arquivo')}</dt>
        <dd className="t-mono t-sm d2__file">{current.fileName}</dd>
        <dt>{t('dependencias.detalhe.fonte')}</dt>
        <dd>
          <SourceDots sources={[{ source: current.source }]} label />
        </dd>
        <dt>{t('dependencias.detalhe.lado')}</dt>
        <dd>
          {current.sideNote === 'unknown' ? (
            <>
              <span>{t('dependencias.detalhe.ladoNaoInformado')}</span>
              <span className="d2__sub">{t('dependencias.detalhe.ladoNaoInformadoSub')}</span>
            </>
          ) : current.sideNote === 'eitherSide' ? (
            t('dependencias.detalhe.qualquerLado')
          ) : (
            t(`lado.${current.side}`)
          )}
        </dd>
      </dl>
    </div>
  );
}

/** O problema do item, com o botão que resolve (incompatível, já no pack, obrigatória fora). */
function Problems({
  plan,
  node,
  alvo,
  included,
  choices,
  onChange,
}: {
  plan: AddPlan;
  node: PlanNode;
  alvo: string;
  included: ReadonlySet<string>;
  choices: DependencyChoices;
  onChange: (choices: DependencyChoices) => void;
}) {
  const { t } = useTranslation('adicionar');
  const blocks: ReactNode[] = [];
  const set = (on: boolean) => {
    onChange(setIncluded(plan, choices, node.key, on));
  };
  const active = activeConflicts(plan, included);
  for (const conflict of plan.conflicts.filter((item) => item.item.key === node.key)) {
    const on = active.includes(conflict);
    const declared =
      conflict.declaredBy === conflict.other.key ? conflict.other.title : conflict.item.title;
    const params = { item: conflict.item.title, other: conflict.other.title };
    blocks.push(
      on ? (
        <div key={conflict.other.key} className="pb pb--danger">
          <Icon icon={OctagonAlert} />
          <span className="pb__t">
            {conflict.withPack
              ? t('dependencias.problema.incompativelPack', params)
              : t('dependencias.problema.incompativelEntre', params)}
          </span>
          <span className="pb__d">
            {t('dependencias.problema.declaradoPor', { name: declared })}
            {conflict.reason
              ? ` ${t('dependencias.problema.motivo', { reason: conflict.reason })}`
              : ''}
          </span>
          <span className="pb__act">
            <Button
              size="sm"
              onClick={() => {
                set(false);
              }}
            >
              {t('dependencias.problema.naoAdicionar', { name: node.title })}
            </Button>
          </span>
        </div>
      ) : (
        <div key={conflict.other.key} className="pb pb--done">
          <Icon icon={Check} />
          <span className="pb__t">
            {t('dependencias.problema.naoVaiEntrar', { name: node.title })}
          </span>
          <span className="pb__d">
            {t('dependencias.problema.naoVaiEntrarTexto', { other: conflict.other.title })}
          </span>
          {included.has(node.key) ? null : (
            <span className="pb__act">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  set(true);
                }}
              >
                {t('dependencias.problema.adicionarMesmoAssim')}
              </Button>
            </span>
          )}
        </div>
      ),
    );
  }
  const duplicate = plan.duplicates.find((item) => item.key === node.key);
  if (duplicate) {
    const replace = choices.replace.has(node.key);
    const pick = (on: boolean) => {
      const next = new Set(choices.replace);
      if (on) next.add(node.key);
      else next.delete(node.key);
      onChange({ ...choices, replace: next });
    };
    const name = `dup-${node.key}`;
    blocks.push(
      <div key="dup" className="pb pb--warn">
        <Icon icon={Copy} />
        <span className="pb__t">
          {t('dependencias.problema.duplicado', {
            name: node.title,
            pela: t(`dependencias.pela.${duplicate.existingSource ?? 'link'}`),
          })}
        </span>
        <span className="pb__d">
          {t('dependencias.problema.duplicadoTexto', { path: duplicate.existingPath })}
        </span>
        <fieldset className="pb__choice">
          <legend className="sr-only">
            {t('dependencias.problema.duplicadoEscolha', { name: node.title })}
          </legend>
          <label className="check">
            <input
              type="radio"
              name={name}
              checked={!replace}
              onChange={() => {
                pick(false);
              }}
            />
            <span>{t('dependencias.problema.manter')}</span>
          </label>
          <label className="check">
            <input
              type="radio"
              name={name}
              checked={replace}
              onChange={() => {
                pick(true);
              }}
            />
            <span>{t('dependencias.problema.trocar', { fonte: t(`fonte.${node.source}`) })}</span>
          </label>
        </fieldset>
      </div>,
    );
  }
  if (
    node.role === 'required' &&
    choices.unchecked.has(node.key) &&
    node.requiredBy.some((key) => included.has(key))
  ) {
    const who = onlyIncluded(node.requiredBy, included);
    blocks.push(
      <div key="req" className="pb pb--danger">
        <Icon icon={OctagonAlert} />
        <span className="pb__t">
          {t('dependencias.problema.semObrigatoria', { name: node.title })}
        </span>
        <span className="pb__d">
          {t('dependencias.problema.semObrigatoriaTexto', {
            count: who.length,
            names: joinNames(titlesOf(plan, who)),
          })}
        </span>
        <span className="pb__act">
          <Button
            size="sm"
            onClick={() => {
              set(true);
            }}
          >
            {t('dependencias.problema.marcarDeNovo')}
          </Button>
        </span>
      </div>,
    );
  }
  if (!node.compatible) {
    blocks.push(
      <div key="ver" className="pb pb--warn">
        <Icon icon={TriangleAlert} />
        <span className="pb__t">{t('dependencias.tag.versaoIncompativel')}</span>
        <span className="pb__d">{t('dependencias.tag.versaoIncompativelDica', { alvo })}</span>
      </div>,
    );
  }
  return blocks.length > 0 ? <div className="pbs">{blocks}</div> : null;
}
