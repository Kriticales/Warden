/**
 * A revisão de "O que mudou durante o teste" (SPEC T15; protótipo `teste-fechou`): os grupos com
 * a pré-seleção, os conflitos, os mods colocados à mão e as ações "Descartar tudo", "Decidir
 * depois" e "Trazer N selecionados para o pack".
 */
import { useNavigate } from '@tanstack/react-router';
import {
  Download,
  FileCode,
  GitCompare,
  Globe,
  Image,
  Puzzle,
  Settings,
  type LucideIcon,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type { CaptureFileView, CaptureView, PackId } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useCaptureApply, useCaptureDiscard } from '../api';
import {
  buildRequest,
  groupsOf,
  initialSelection,
  packChangedKeys,
  selectedCount,
  type Selection,
} from '../model';
import { ChangeItem } from './ChangeItem';
import { ConflictItem } from './ConflictItem';
import { KeyDiff } from './KeyDiff';
import { ManualItems } from './ManualItems';
import { TextDiffDialog } from './TextDiffDialog';

export interface CaptureReviewProps {
  packId: PackId;
  view: CaptureView;
  /** Trouxe para o pack (quantas mudanças). */
  onApplied: (count: number) => void;
}

function Group({
  icon,
  title,
  count,
  children,
}: {
  icon: LucideIcon;
  title: string;
  count: number;
  children: ReactNode;
}) {
  const { t } = useTranslation('captura');
  return (
    <section className="change-group">
      <h3>
        <Icon icon={icon} className="t-3" />
        {title} <span className="t-3">{t('contagem', { count })}</span>
      </h3>
      {children}
    </section>
  );
}

export function CaptureReview({ packId, view, onApplied }: CaptureReviewProps) {
  const { t } = useTranslation('captura');
  const navigate = useNavigate();
  const [selection, setSelection] = useState<Selection>(() => initialSelection(view));
  const [diffPath, setDiffPath] = useState<string | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const apply = useCaptureApply(packId, {
    onSuccess: (result) => {
      onApplied(result.count);
    },
    onError: (error) => {
      showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
    },
  });
  const discard = useCaptureDiscard(packId);
  const groups = groupsOf(view);
  const count = selectedCount(selection);
  const changedInPack = packChangedKeys(groups.options);
  const titleId = `capture-${view.sessionId}`;

  const setFile = (path: string, checked: boolean) => {
    setSelection((current) => ({ ...current, files: { ...current.files, [path]: checked } }));
  };
  const diffOf = (file: CaptureFileView) =>
    file.file.text
      ? () => {
          setDiffPath(file.file.path);
        }
      : undefined;

  const fileItem = (file: CaptureFileView, detail: ReactNode) => (
    <ChangeItem
      key={file.file.path}
      name={file.file.path}
      label={file.file.path}
      checked={selection.files[file.file.path] === true}
      onCheck={(checked) => {
        setFile(file.file.path, checked);
      }}
      onDiff={diffOf(file)}
    >
      {detail}
    </ChangeItem>
  );

  const valuesOf = (file: CaptureFileView) => {
    if (file.file.change === 'removed') return <p className="t-xs t-2">{t('item.apagado')}</p>;
    if (file.file.values.length > 0) return <KeyDiff rows={file.file.values} />;
    if (file.file.change === 'added') return <p className="t-xs t-2">{t('item.criado')}</p>;
    return <p className="t-xs t-2">{t('item.semValores')}</p>;
  };

  const onApply = () => {
    apply.mutate(buildRequest(view, selection));
  };

  const manual = [...groups.byGroup.mod, ...groups.byGroup.resource, ...groups.byGroup.datapack];
  const options = groups.options;

  return (
    <section className="panel panel--strong capture" aria-labelledby={titleId}>
      <div className="panel__head">
        <div>
          <h2 className="panel__title" id={titleId}>
            {t('titulo')}
          </h2>
          <p className="t-sm t-3">{t('resumo', { count: view.files.length })}</p>
        </div>
      </div>
      <div className="changes">
        {groups.conflicts.length > 0 ? (
          <section className="change-group">
            <h3>
              <Icon icon={GitCompare} className="t-warn" />
              {t('grupos.conflito')}{' '}
              <span className="t-3">{t('contagem', { count: groups.conflicts.length })}</span>
            </h3>
            {groups.conflicts.map((file) => (
              <ConflictItem
                key={file.file.path}
                packId={packId}
                sessionId={view.sessionId}
                view={file}
                choice={selection.conflicts[file.file.path] ?? { kind: 'keepPack' }}
                onChoose={(choice) => {
                  setSelection((current) => ({
                    ...current,
                    conflicts: { ...current.conflicts, [file.file.path]: choice },
                  }));
                }}
              />
            ))}
          </section>
        ) : null}
        {groups.byGroup.packConfig.length > 0 ? (
          <Group
            icon={FileCode}
            title={t('grupos.packConfig')}
            count={groups.byGroup.packConfig.length}
          >
            {groups.byGroup.packConfig.map((file) => fileItem(file, valuesOf(file)))}
          </Group>
        ) : null}
        {options ? (
          <Group icon={Settings} title={t('grupos.options')} count={options.file.options.length}>
            {options.file.options.map((option) => {
              const inPack = changedInPack.get(option.key);
              return (
                <ChangeItem
                  key={option.key}
                  name={option.key}
                  label={option.key}
                  checked={selection.options[option.key] === true}
                  onCheck={(checked) => {
                    setSelection((current) => ({
                      ...current,
                      options: { ...current.options, [option.key]: checked },
                    }));
                  }}
                >
                  <KeyDiff
                    rows={[{ key: option.key, before: option.before, after: option.after }]}
                  />
                  {inPack !== undefined ? (
                    <p className="t-xs t-warn">{t('opcoes.noPack', { valor: inPack })}</p>
                  ) : null}
                </ChangeItem>
              );
            })}
            <div className="row row--wrap capture-note">
              {options.file.hiddenVolatile > 0 ? (
                <p className="t-xs t-3">
                  {t('opcoes.escondidas', { count: options.file.hiddenVolatile })}
                </p>
              ) : null}
              {options.file.text ? (
                <Button
                  variant="link"
                  aria-label={t('item.verDiferencaDe', { nome: options.file.path })}
                  onClick={() => {
                    setDiffPath(options.file.path);
                  }}
                >
                  {t('item.verDiferenca')}
                </Button>
              ) : null}
            </div>
          </Group>
        ) : null}
        {groups.byGroup.worldConfig.length > 0 ? (
          <Group
            icon={Globe}
            title={t('grupos.worldConfig')}
            count={groups.byGroup.worldConfig.length}
          >
            {groups.byGroup.worldConfig.map((file) =>
              fileItem(
                file,
                <>
                  {file.file.values.length > 0 ? <KeyDiff rows={file.file.values} /> : null}
                  <p className="t-xs t-2">
                    {t('item.destinoMundo', { destino: file.file.packPath ?? '' })}
                  </p>
                </>,
              ),
            )}
          </Group>
        ) : null}
        {manual.length > 0 ? (
          <Group
            icon={manual.some((item) => item.file.group === 'mod') ? Puzzle : Image}
            title={
              groups.byGroup.mod.length > 0
                ? t('grupos.mod')
                : groups.byGroup.resource.length > 0
                  ? t('grupos.resource')
                  : t('grupos.datapack')
            }
            count={manual.length}
          >
            <ManualItems packId={packId} sessionId={view.sessionId} items={manual} />
          </Group>
        ) : null}
        {groups.byGroup.generated.length > 0 ? (
          <details className="disclosure">
            <summary>{t('grupos.generated', { count: groups.byGroup.generated.length })}</summary>
            <div className="capture-generated">
              {groups.byGroup.generated.map((file) => fileItem(file, valuesOf(file)))}
            </div>
          </details>
        ) : null}
        {view.reformatted > 0 ? (
          <p className="t-xs t-3">{t('reformatados', { count: view.reformatted })}</p>
        ) : null}
      </div>
      <div className="capture__foot">
        <Button
          variant="danger-ghost"
          onClick={() => {
            setConfirmDiscard(true);
          }}
        >
          {t('acoes.descartar')}
        </Button>
        <div className="row row--wrap">
          <Button
            variant="ghost"
            onClick={() => {
              void navigate({ to: '/packs/$packId/mods', params: { packId } });
            }}
          >
            {t('acoes.depois')}
          </Button>
          <Button
            variant="primary"
            icon={Download}
            loading={apply.isPending}
            disabled={count === 0}
            onClick={onApply}
          >
            {apply.isPending ? t('acoes.trazendo') : t('acoes.trazer', { count })}
          </Button>
        </div>
      </div>
      {diffPath !== null ? (
        <TextDiffDialog
          packId={packId}
          sessionId={view.sessionId}
          path={diffPath}
          onClose={() => {
            setDiffPath(null);
          }}
        />
      ) : null}
      <ConfirmDialog
        open={confirmDiscard}
        onOpenChange={setConfirmDiscard}
        title={t('descartar.titulo')}
        description={t('descartar.texto')}
        confirmLabel={t('descartar.confirmar')}
        confirmingLabel={t('descartar.descartando')}
        onConfirm={async () => {
          await discard.mutateAsync(view.sessionId);
        }}
      />
    </section>
  );
}
