/**
 * Trocar a versão do loader do pack, sem trocar de loader (SPEC T11, "Editar informações";
 * P1-15). Ao escolher a versão, o Warden confere cada mod contra ela (a faixa de versões do
 * loader que o mod declara) e mostra **antes** o que para de funcionar; nada muda sem
 * confirmar. Depois de gravar, a mesma conferência roda de novo contra a versão gravada.
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
  Loader,
  LoaderCheck,
  LoaderIssue,
  LoaderVersion,
  PackId,
} from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';
import { useLoaderCheck, useLoaderVersions, useSetLoaderVersion } from './api';

/** Loaders cuja lista de versões o catálogo conhece. */
const CATALOG_LOADERS: readonly string[] = ['forge', 'neoforge', 'fabric'];

export function isCatalogLoader(loader: string | null): loader is Loader {
  return loader !== null && CATALOG_LOADERS.includes(loader);
}

export interface LoaderVersionDialogProps {
  packId: PackId;
  loader: Loader;
  minecraft: string;
  /** A versão gravada no `pack.toml` agora. */
  current: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Quem abriu o diálogo: recebe o foco de volta ao fechar. */
  returnFocusTo: () => HTMLElement | null;
}

export function LoaderVersionDialog({
  packId,
  loader,
  minecraft,
  current,
  open,
  onOpenChange,
  returnFocusTo,
}: LoaderVersionDialogProps) {
  const { t } = useTranslation('trocarVersao');
  const name = loaderName(loader);
  const versions = useLoaderVersions(loader, minecraft, open);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        versions.isSuccess ? (
          <LoaderForm
            packId={packId}
            loader={loader}
            current={current}
            versions={versions.data.versions}
            returnFocusTo={returnFocusTo}
            onDone={() => {
              onOpenChange(false);
            }}
          />
        ) : (
          <DialogContent title={t('loader.titulo', { loader: name })} returnFocusTo={returnFocusTo}>
            {versions.isError ? (
              <ErrorPanel
                error={versions.error}
                onRetry={() => {
                  void versions.refetch();
                }}
              />
            ) : (
              <LoadingState inline label={t('loader.carregando', { loader: name })} />
            )}
          </DialogContent>
        )
      ) : null}
    </Dialog>
  );
}

function LoaderForm({
  packId,
  loader,
  current,
  versions,
  returnFocusTo,
  onDone,
}: {
  packId: PackId;
  loader: Loader;
  current: string;
  versions: LoaderVersion[];
  returnFocusTo: () => HTMLElement | null;
  onDone: () => void;
}) {
  const { t } = useTranslation('trocarVersao');
  const id = useId();
  const name = loaderName(loader);
  const [selected, setSelected] = useState(current);
  const other = selected !== current;
  const check = useLoaderCheck(packId, other ? selected : null);
  const set = useSetLoaderVersion(packId);
  // A versão do pack pode não estar no catálogo (versão antiga ou escrita à mão).
  const listed = versions.some((version) => version.version === current)
    ? versions
    : [{ version: current, maven: '', stable: true, recommended: false }, ...versions];
  const ready = other && check.isSuccess && check.data.to === selected;

  const label = (version: LoaderVersion) =>
    version.version === current
      ? t('loader.atual', { versao: version.version })
      : version.recommended
        ? t('loader.recomendada', { versao: version.version })
        : version.stable
          ? version.version
          : t('loader.instavel', { versao: version.version });

  const confirm = () => {
    if (!ready) return;
    set.mutate(
      { expectedFrom: current, version: selected },
      {
        onSuccess: (result) => {
          const problems = result.check.breaks.length + result.check.stillBroken.length;
          showToast(
            problems > 0
              ? {
                  kind: 'warn',
                  title: t('loader.feitoComProblemas', {
                    loader: name,
                    versao: result.changed.to,
                    count: problems,
                  }),
                }
              : {
                  kind: 'ok',
                  title: t('loader.feito', { loader: name, versao: result.changed.to }),
                },
          );
          onDone();
        },
      },
    );
  };

  return (
    <DialogContent
      title={t('loader.titulo', { loader: name })}
      description={t('loader.descricao')}
      returnFocusTo={returnFocusTo}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('loader.cancelar')}</Button>
          </DialogClose>
          <Button variant="primary" disabled={!ready} loading={set.isPending} onClick={confirm}>
            {set.isPending ? t('loader.trocando') : t('loader.trocar', { versao: selected })}
          </Button>
        </>
      }
    >
      <div className="stack-3">
        <div className="field">
          <label className="field__label" htmlFor={id}>
            {t('loader.versao', { loader: name })}
          </label>
          <select
            id={id}
            className="select"
            value={selected}
            onChange={(event) => {
              setSelected(event.target.value);
            }}
          >
            {listed.map((version) => (
              <option key={version.version} value={version.version}>
                {label(version)}
              </option>
            ))}
          </select>
        </div>
        {!other ? <p className="t-sm t-3">{t('loader.mesma')}</p> : null}
        {other && check.isPending ? (
          <LoadingState inline label={t('loader.conferindo', { versao: selected })} />
        ) : null}
        {check.isError ? (
          <ErrorPanel
            compact
            error={check.error}
            onRetry={() => {
              void check.refetch();
            }}
          />
        ) : null}
        {ready ? <CheckResult check={check.data} /> : null}
        {set.isError ? <ErrorPanel compact error={set.error} /> : null}
      </div>
    </DialogContent>
  );
}

function IssueList({ issues }: { issues: LoaderIssue[] }) {
  const { t } = useTranslation('trocarVersao');
  return (
    <ul className="alert__text">
      {issues.map((issue) => (
        <li key={issue.path}>
          {issue.requires
            ? t('loader.exige', { name: issue.name, faixa: issue.requires })
            : issue.name}
        </li>
      ))}
    </ul>
  );
}

function CheckResult({ check }: { check: LoaderCheck }) {
  const { t } = useTranslation('trocarVersao');
  const versao = check.to;
  return (
    <>
      {check.breaks.length > 0 ? (
        <Alert
          kind="danger"
          compact
          title={t('loader.quebraTitulo', { count: check.breaks.length, versao })}
        >
          <IssueList issues={check.breaks} />
        </Alert>
      ) : null}
      {check.stillBroken.length > 0 ? (
        <Alert
          kind="warn"
          compact
          title={t('loader.continuaTitulo', { count: check.stillBroken.length })}
        >
          <IssueList issues={check.stillBroken} />
        </Alert>
      ) : null}
      {check.fixes.length > 0 ? (
        <Alert kind="ok" compact title={t('loader.voltaTitulo', { count: check.fixes.length })}>
          <IssueList issues={check.fixes} />
        </Alert>
      ) : null}
      {check.breaks.length === 0 && check.stillBroken.length === 0 && check.checked > 0 ? (
        <p className="t-sm">{t('loader.tudoCerto', { count: check.checked, versao })}</p>
      ) : null}
      {check.unchecked.length > 0 ? (
        <p className="t-sm t-3">
          {t('loader.naoConferidos', {
            count: check.unchecked.length,
            nomes: check.unchecked.join(', '),
          })}
        </p>
      ) : null}
    </>
  );
}
