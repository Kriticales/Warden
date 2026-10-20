/**
 * "Trocar de versão" de um item (SPEC T07, **Mais opções**; P1-15): qualquer versão compatível
 * com o pack, inclusive mais antiga, com aviso claro de rebaixamento, das dependências que a
 * versão pede e das incompatibilidades com itens do pack. Nada muda até confirmar.
 */
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import type {
  ChangeVersionOptions,
  InventoryItem,
  PackId,
  VersionChoice,
} from '../../../lib/ipc/bindings';
import { formatDate } from '../../../lib/format';
import { describedBy } from '../../settings/components/fields';
import { useChangeVersion, useVersionOptions, useVersionPlan } from './api';

export interface ChangeVersionDialogProps {
  packId: PackId;
  item: InventoryItem;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Quem abriu o diálogo: recebe o foco de volta ao fechar. */
  returnFocusTo: () => HTMLElement | null;
  /** Depois de trocar a versão (o Diagnóstico verifica o pack de novo). */
  onChanged?: (() => void) | undefined;
}

export function ChangeVersionDialog({
  packId,
  item,
  open,
  onOpenChange,
  returnFocusTo,
  onChanged,
}: ChangeVersionDialogProps) {
  const { t } = useTranslation('trocarVersao');
  const options = useVersionOptions(packId, item.path, open);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        options.isSuccess ? (
          <VersionForm
            packId={packId}
            options={options.data}
            returnFocusTo={returnFocusTo}
            onDone={() => {
              onOpenChange(false);
              onChanged?.();
            }}
          />
        ) : (
          <DialogContent
            title={t('dialogo.titulo', { name: item.name })}
            returnFocusTo={returnFocusTo}
          >
            {options.isError ? (
              <ErrorPanel
                error={options.error}
                onRetry={() => {
                  void options.refetch();
                }}
              />
            ) : (
              <LoadingState inline label={t('dialogo.carregando')} />
            )}
          </DialogContent>
        )
      ) : null}
    </Dialog>
  );
}

/** O texto de uma versão na lista: número, data e as marcas (instalada, mais antiga, canal). */
function useOptionLabel() {
  const { t } = useTranslation('trocarVersao');
  return (version: VersionChoice) => {
    const marks: string[] = [];
    if (version.installed) marks.push(t('dialogo.marca.instalada'));
    if (version.older) marks.push(t('dialogo.marca.maisAntiga'));
    if (version.channel !== 'release') marks.push(t(`dialogo.marca.${version.channel}`));
    const published = Date.parse(version.published);
    const data = Number.isNaN(published) ? '' : formatDate(published);
    return marks.length > 0
      ? t('dialogo.opcaoComMarcas', { versao: version.number, data, marcas: marks.join(', ') })
      : t('dialogo.opcao', { versao: version.number, data });
  };
}

function VersionForm({
  packId,
  options,
  returnFocusTo,
  onDone,
}: {
  packId: PackId;
  options: ChangeVersionOptions;
  returnFocusTo: () => HTMLElement | null;
  onDone: () => void;
}) {
  const { t } = useTranslation('trocarVersao');
  const id = useId();
  const label = useOptionLabel();
  const installed = options.versions.find((version) => version.installed);
  // Começa na instalada (nada a trocar) ou, se ela não serve mais, na mais nova.
  const [selected, setSelected] = useState<string | null>(
    installed?.id ?? options.versions[0]?.id ?? null,
  );
  const plan = useVersionPlan(packId, options.path, selected);
  const change = useChangeVersion(packId);
  const chosen = options.versions.find((version) => version.id === selected);
  const ready = plan.isSuccess && plan.data.to.id === selected && !plan.data.same;
  const downgrade = ready && plan.data.downgrade;
  const hint = options.current ? t('dialogo.instalada', { versao: options.current }) : null;

  const confirm = () => {
    if (!ready || !chosen) return;
    change.mutate(
      { path: options.path, expectedCurrentId: options.currentId, versionId: chosen.id },
      {
        onSuccess: (changed) => {
          showToast({
            kind: 'ok',
            title: t('dialogo.feito', { name: changed.name, versao: changed.to }),
          });
          onDone();
        },
      },
    );
  };

  return (
    <DialogContent
      title={t('dialogo.titulo', { name: options.name })}
      description={t('dialogo.descricao')}
      returnFocusTo={returnFocusTo}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('dialogo.cancelar')}</Button>
          </DialogClose>
          <Button
            variant={downgrade ? 'danger' : 'primary'}
            disabled={!ready}
            loading={change.isPending}
            onClick={confirm}
          >
            {change.isPending
              ? t('dialogo.trocando')
              : downgrade
                ? t('dialogo.rebaixar', { versao: chosen?.number ?? '' })
                : t('dialogo.trocar', { versao: chosen?.number ?? '' })}
          </Button>
        </>
      }
    >
      <div className="stack-3">
        {options.currentIncompatible ? (
          <Alert kind="warn" compact>
            <div className="alert__text">{t('dialogo.atualIncompativel')}</div>
          </Alert>
        ) : null}
        {options.versions.length === 0 ? (
          <Alert kind="info" compact>
            <div className="alert__text">{t('dialogo.semVersoes')}</div>
          </Alert>
        ) : (
          <div className="field">
            <label className="field__label" htmlFor={id}>
              {t('dialogo.versao')}
            </label>
            <select
              id={id}
              className="select"
              value={selected ?? ''}
              aria-describedby={describedBy(id, hint, null)}
              onChange={(event) => {
                setSelected(event.target.value);
              }}
            >
              {options.versions.map((version) => (
                <option key={version.id} value={version.id}>
                  {label(version)}
                </option>
              ))}
            </select>
            {hint ? (
              <div className="field__hint" id={`${id}-hint`}>
                {hint}
              </div>
            ) : null}
          </div>
        )}
        {options.pinned ? <p className="t-sm t-3">{t('dialogo.fixado')}</p> : null}
        {selected !== null && plan.isPending ? (
          <LoadingState inline label={t('dialogo.conferindo')} />
        ) : null}
        {plan.isError ? (
          <ErrorPanel
            compact
            error={plan.error}
            onRetry={() => {
              void plan.refetch();
            }}
          />
        ) : null}
        {plan.isSuccess ? <PlanNotes plan={plan.data} /> : null}
        {change.isError ? <ErrorPanel compact error={change.error} /> : null}
      </div>
    </DialogContent>
  );
}

function PlanNotes({ plan }: { plan: NonNullable<ReturnType<typeof useVersionPlan>['data']> }) {
  const { t } = useTranslation('trocarVersao');
  if (plan.same) {
    return <p className="t-sm t-3">{t('dialogo.mesma')}</p>;
  }
  return (
    <>
      {plan.downgrade ? (
        <Alert kind="warn" compact title={t('dialogo.rebaixamentoTitulo')}>
          <div className="alert__text">
            {t('dialogo.rebaixamento', { atual: plan.current ?? '', nova: plan.to.number })}
          </div>
        </Alert>
      ) : null}
      {plan.newDependencies.length > 0 ? (
        <Alert
          kind="warn"
          compact
          title={t('dialogo.dependenciasTitulo', { count: plan.newDependencies.length })}
        >
          <div className="alert__text">
            {t('dialogo.dependencias', {
              nomes: plan.newDependencies.map((dependency) => dependency.name).join(', '),
            })}
          </div>
        </Alert>
      ) : null}
      {plan.conflicts.length > 0 ? (
        <Alert kind="danger" compact title={t('dialogo.conflitosTitulo')}>
          <div className="alert__text">
            {t('dialogo.conflitos', {
              nomes: plan.conflicts.map((conflict) => conflict.with).join(', '),
            })}
          </div>
        </Alert>
      ) : null}
    </>
  );
}
