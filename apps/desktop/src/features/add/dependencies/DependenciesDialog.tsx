/**
 * Diálogo de dependências (SPEC T09; protótipo `depsDialog`): aparece antes de gravar qualquer
 * coisa, para um item ou vários de uma vez (seleção múltipla, e depois kits, modpacks e mods
 * iniciais). Mostra o que você escolheu, as obrigatórias (marcadas; desmarcar avisa), o que já
 * está no pack, as opcionais (desmarcadas), o que não tem versão para o pack, as
 * incompatibilidades declaradas (com o pack e entre os próprios itens) e o mesmo mod no pack por
 * outra fonte (substituir ou não adicionar). **Adicionar N itens** grava tudo de uma vez:
 * ou todos entram, ou nenhum.
 */
import { CircleCheck, Plus, TriangleAlert } from 'lucide-react';
import { useId, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type {
  AddChoice,
  AddPlan,
  AddResult,
  PackId,
  PlanConflict,
  PlanNode,
  SourceId,
} from '../../../lib/ipc/bindings';
import { useAddApply, useAddPlan } from '../common/api';
import { type PackTarget, useTargetText } from '../common/text';
import {
  applyRequest,
  type DependencyChoices,
  includedKeys,
  joinNames,
  NO_CHOICES,
  nodesOf,
  titlesOf,
} from './model';

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
  const plan = useAddPlan(packId, request?.choices ?? null);
  const apply = useAddApply(packId);
  const [choices, setChoices] = useState<DependencyChoices>(NO_CHOICES);
  const included = useMemo(
    () => (plan.data ? includedKeys(plan.data, choices) : new Set<string>()),
    [plan.data, choices],
  );

  const title =
    request === null
      ? ''
      : request.titles.length === 1
        ? t('dependencias.tituloUm', { name: request.titles[0] ?? '' })
        : t('dependencias.tituloVarios', { count: request.titles.length });

  const close = () => {
    if (apply.isPending) return;
    setChoices(NO_CHOICES);
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
          size="lg"
          title={title}
          description={t('dependencias.sub')}
          footer={
            <>
              <Button variant="ghost" onClick={close} disabled={apply.isPending}>
                {t('acoes.cancelar', { ns: 'comum' })}
              </Button>
              <Button
                variant="primary"
                icon={Plus}
                loading={apply.isPending}
                disabled={!plan.data || included.size === 0}
                onClick={() => {
                  if (plan.data) confirm(plan.data);
                }}
              >
                {apply.isPending
                  ? t('dependencias.adicionando')
                  : t('dependencias.adicionar', { count: included.size })}
              </Button>
            </>
          }
        >
          {plan.isPending ? (
            <LoadingState label={t('dependencias.carregando')} />
          ) : plan.isError ? (
            <ErrorPanel
              title={t('dependencias.erro')}
              error={plan.error}
              onRetry={() => {
                void plan.refetch();
              }}
            />
          ) : (
            <PlanView
              plan={plan.data}
              target={target}
              included={included}
              choices={choices}
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

function toggle(set: ReadonlySet<string>, key: string, on: boolean): Set<string> {
  const next = new Set(set);
  if (on) next.add(key);
  else next.delete(key);
  return next;
}

interface PlanViewProps {
  plan: AddPlan;
  target: PackTarget;
  included: ReadonlySet<string>;
  choices: DependencyChoices;
  onChange: (choices: DependencyChoices) => void;
}

function PlanView({ plan, target, included, choices, onChange }: PlanViewProps) {
  const { t } = useTranslation('adicionar');
  const targetText = useTargetText(target);
  const chosen = nodesOf(plan, 'chosen');
  const required = nodesOf(plan, 'required');
  const optional = nodesOf(plan, 'optional');
  const names = (keys: readonly string[]) => joinNames(titlesOf(plan, keys));

  return (
    <div className="stack">
      {chosen.length > 0 ? (
        <Group title={t('dependencias.escolhidos')}>
          {chosen.map((node) => (
            <NodeCheck
              key={node.key}
              node={node}
              targetText={targetText}
              checked={included.has(node.key)}
              disabled={plan.duplicates.some(
                (duplicate) => duplicate.key === node.key && !choices.replace.has(node.key),
              )}
              onChange={(on) => {
                onChange({ ...choices, unchecked: toggle(choices.unchecked, node.key, !on) });
              }}
            />
          ))}
        </Group>
      ) : null}

      {plan.duplicates.map((duplicate) => (
        <DuplicateChoice
          key={duplicate.key}
          title={titlesOf(plan, [duplicate.key])[0] ?? duplicate.key}
          existingTitle={duplicate.existingTitle}
          existingPath={duplicate.existingPath}
          existingSource={duplicate.existingSource}
          source={plan.nodes.find((node) => node.key === duplicate.key)?.source ?? 'modrinth'}
          replace={choices.replace.has(duplicate.key)}
          onChange={(replace) => {
            onChange({ ...choices, replace: toggle(choices.replace, duplicate.key, replace) });
          }}
        />
      ))}

      {required.length > 0 ? (
        <Group title={t('dependencias.obrigatorias')}>
          {required.map((node) => {
            const requested = node.requiredBy.some((key) => included.has(key));
            const unchecked = choices.unchecked.has(node.key);
            return (
              <NodeCheck
                key={node.key}
                node={node}
                targetText={targetText}
                checked={included.has(node.key)}
                disabled={!requested}
                description={t('dependencias.pedidaPor', { names: names(node.requiredBy) })}
                warning={requested && unchecked ? t('dependencias.semObrigatoria') : null}
                onChange={(on) => {
                  onChange({ ...choices, unchecked: toggle(choices.unchecked, node.key, !on) });
                }}
              />
            );
          })}
        </Group>
      ) : null}

      {plan.installed.length > 0 ? (
        <Group title={t('dependencias.jaNoPack')}>
          <ul className="stack-2">
            {plan.installed.map((item) => {
              const wanted = item.key.split(':')[0];
              const other = item.packSource !== wanted;
              return (
                <li key={item.key} className="row t-sm deps__installed">
                  <Icon icon={CircleCheck} size="sm" />
                  <span>
                    <b>{item.title}</b>
                    {other ? ` ${t(`dependencias.pelaFonte.${item.packSource ?? 'link'}`)}` : ''}
                  </span>
                </li>
              );
            })}
          </ul>
        </Group>
      ) : null}

      {optional.length > 0 ? (
        <Group title={t('dependencias.opcionais')}>
          {optional.map((node) => (
            <NodeCheck
              key={node.key}
              node={node}
              targetText={targetText}
              checked={included.has(node.key)}
              disabled={!node.optionalFor.some((key) => included.has(key))}
              description={t('dependencias.opcionalPara', { names: names(node.optionalFor) })}
              onChange={(on) => {
                onChange({ ...choices, optional: toggle(choices.optional, node.key, on) });
              }}
            />
          ))}
        </Group>
      ) : null}

      {plan.missing.length > 0 ? (
        <Group title={t('dependencias.semVersao')}>
          <ul className="stack-2">
            {plan.missing.map((item) => (
              <li key={item.key} className="row row--top t-sm">
                <Icon icon={TriangleAlert} size="sm" className="t-warn" />
                <span>
                  {t('dependencias.semVersaoItem', { name: item.title, alvo: targetText })}{' '}
                  <span className="t-3">
                    {item.role === 'chosen'
                      ? t('dependencias.semVersaoEscolhido')
                      : t('dependencias.semVersaoDependencia')}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        </Group>
      ) : null}

      {plan.conflicts.length > 0 ? (
        plan.conflicts.map((conflict) => (
          <ConflictAlert key={`${conflict.item.key}|${conflict.other.key}`} conflict={conflict} />
        ))
      ) : (
        <Alert kind="ok" compact title={t('dependencias.semIncompatibilidade')}>
          <p>{t('dependencias.semIncompatibilidadeTexto', { count: plan.packItemCount })}</p>
        </Alert>
      )}
    </div>
  );
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  const id = useId();
  return (
    <section className="deps__group" aria-labelledby={id}>
      <h3 className="t-caps t-3" id={id}>
        {title}
      </h3>
      <div className="stack-2">{children}</div>
    </section>
  );
}

interface NodeCheckProps {
  node: PlanNode;
  targetText: string;
  checked: boolean;
  disabled?: boolean;
  description?: string;
  warning?: string | null;
  onChange: (checked: boolean) => void;
}

function NodeCheck({
  node,
  targetText,
  checked,
  disabled = false,
  description,
  warning = null,
  onChange,
}: NodeCheckProps) {
  const { t } = useTranslation('adicionar');
  const notes = [
    description,
    node.outsideChannel
      ? t('dependencias.versaoForaDoCanal', { canal: t(`dependencias.canal.${node.channel}`) })
      : null,
    node.compatible ? null : t('dependencias.versaoIncompativel', { alvo: targetText }),
    node.sideNote === 'unknown'
      ? t('dependencias.ladoDesconhecido')
      : node.sideNote === 'eitherSide'
        ? t('dependencias.qualquerLado')
        : null,
  ].filter((note): note is string => Boolean(note));
  return (
    <div>
      <label className="check">
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(event) => {
            onChange(event.target.checked);
          }}
        />
        <span className="check__text">
          <span>
            <b>{node.title}</b> <span className="t-mono t-sm">{node.versionNumber}</span>
          </span>
          {notes.map((note) => (
            <span key={note} className="check__desc">
              {note}
            </span>
          ))}
        </span>
      </label>
      {warning ? (
        <p className="field__error" role="status">
          <Icon icon={TriangleAlert} size="sm" />
          <span>{warning}</span>
        </p>
      ) : null}
    </div>
  );
}

interface DuplicateChoiceProps {
  title: string;
  existingTitle: string;
  existingPath: string;
  existingSource: SourceId | null;
  source: SourceId;
  replace: boolean;
  onChange: (replace: boolean) => void;
}

function DuplicateChoice({
  title,
  existingTitle,
  existingPath,
  existingSource,
  source,
  replace,
  onChange,
}: DuplicateChoiceProps) {
  const { t } = useTranslation('adicionar');
  const name = useId();
  return (
    <Alert kind="warn" title={t(`dependencias.duplicado.${existingSource ?? 'link'}`)}>
      <p>{t('dependencias.duplicadoTexto', { name: existingTitle, path: existingPath })}</p>
      <fieldset className="stack-2">
        <legend className="sr-only">{t('dependencias.duplicadoEscolha', { name: title })}</legend>
        <label className="check">
          <input
            type="radio"
            name={name}
            checked={replace}
            onChange={() => {
              onChange(true);
            }}
          />
          <span>{t('dependencias.substituir', { fonte: t(`fonte.${source}`) })}</span>
        </label>
        <label className="check">
          <input
            type="radio"
            name={name}
            checked={!replace}
            onChange={() => {
              onChange(false);
            }}
          />
          <span>{t('dependencias.naoAdicionar')}</span>
        </label>
      </fieldset>
    </Alert>
  );
}

function ConflictAlert({ conflict }: { conflict: PlanConflict }) {
  const { t } = useTranslation('adicionar');
  const declaredBy =
    conflict.declaredBy === conflict.other.key ? conflict.other.title : conflict.item.title;
  const params = { item: conflict.item.title, other: conflict.other.title };
  return (
    <Alert kind="danger" title={t('dependencias.incompativel')}>
      <p>
        {conflict.withPack
          ? t('dependencias.incompativelPack', params)
          : t('dependencias.incompativelEntre', params)}
      </p>
      <p className="t-sm t-3">{t('dependencias.declaradoPor', { name: declaredBy })}</p>
      {conflict.reason ? (
        <p className="t-sm">{t('dependencias.motivo', { reason: conflict.reason })}</p>
      ) : null}
    </Alert>
  );
}
