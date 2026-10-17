/**
 * Abrir pack existente (SPEC T04; protótipo `abrir-existente`), num diálogo por cima de Meus
 * packs (decisão D38, relato do dono na issue #1). A pasta escolhida em "Abrir ou importar…" é
 * verificada sem escrita (`pack_import_preview`): loader suportado, estado do git, higiene e
 * arquivos de controle. Só "Abrir pack", "Agora não" ou "Limpar N arquivos e abrir" escrevem:
 * registram o pack (`pack_import`) e, com Limpar, apagam os marcados depois de um ponto de
 * segurança (`pack_hygiene_fix`). Enquanto escreve, o diálogo não fecha.
 */
import { useNavigate } from '@tanstack/react-router';
import { FolderOpen } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../components/common/DiffView';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { NameTile } from '../../../components/common/PixelArt';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import type { ImportPreview, PackId } from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { useHygieneFix, useImportPack, useImportPreview } from '../api';
import { useOpenOrImport } from '../hooks/useOpenOrImport';
import { loaderName } from '../lib/pack-list';
import { HygieneTable } from './HygieneTable';
import '../packs.css';

export interface OpenPackDialogProps {
  /** A pasta escolhida no diálogo nativo. */
  path: string;
  /** Fecha o diálogo (volta para Meus packs). */
  onClose: () => void;
  /** Para onde o foco volta ao fechar, se o botão que abriu não estava focado. */
  returnFocusTo: () => HTMLElement | null;
}

export function OpenPackDialog({ path, onClose, returnFocusTo }: OpenPackDialogProps) {
  const { t } = useTranslation('packs');
  const navigate = useNavigate();
  const preview = useImportPreview(path);
  const openOrImport = useOpenOrImport();
  const importPack = useImportPack();
  const fix = useHygieneFix();
  // `null`: todos os arquivos da verificação marcados (o padrão).
  const [chosen, setChosen] = useState<ReadonlySet<string> | null>(null);
  const [addControls, setAddControls] = useState(true);
  const [imported, setImported] = useState<PackId | null>(null);
  const busy = importPack.isPending || fix.isPending;

  const chooseOther = (
    <Button
      size="sm"
      icon={FolderOpen}
      loading={openOrImport.pending === 'folder'}
      onClick={openOrImport.chooseFolder}
    >
      {t('abrir.escolherOutra')}
    </Button>
  );
  const cancel = (
    <Button variant="ghost" disabled={busy} onClick={onClose}>
      {t('abrir.cancelar')}
    </Button>
  );

  const openPack = (packId: PackId) => {
    void navigate({ to: '/packs/$packId', params: { packId } });
  };

  let body: ReactNode;
  let footer: ReactNode = cancel;
  if (preview.isPending) {
    body = <ProgressBar value={null} label={t('abrir.lendo')} meta={t('abrir.lendoMeta')} />;
  } else if (preview.isError) {
    body = <PreviewError error={preview.error} action={chooseOther} />;
  } else {
    const data = preview.data;
    const selected = chosen ?? new Set(data.hygiene.map((item) => item.path));
    const readOnly = data.readOnlyReason !== null;
    const canClean = !readOnly && data.hygiene.length > 0;

    const run = (clean: boolean) => {
      const paths = clean ? [...selected] : [];
      importPack.mutate(
        { path: data.path, addControls: addControls && !readOnly },
        {
          onSuccess: (pack) => {
            setImported(pack.id);
            if (pack.copiedIdReplaced) showToast({ kind: 'warn', title: t('abrir.copia') });
            if (pack.requiredIgnoreAdded.length > 0) {
              showToast({
                kind: 'info',
                title: t('abrir.obrigatorias', { linhas: pack.requiredIgnoreAdded.join(', ') }),
              });
            }
            if (paths.length === 0) {
              showToast({ kind: 'ok', title: t('abrir.feito', { name: data.name }) });
              openPack(pack.id);
              return;
            }
            fix.mutate(
              { packId: pack.id, paths },
              {
                onSuccess: (deleted) => {
                  showToast({ kind: 'ok', title: t('abrir.limpos', { count: deleted.length }) });
                  openPack(pack.id);
                },
              },
            );
          },
        },
      );
    };

    body = (
      <PreviewReady
        preview={data}
        selected={selected}
        onSelectedChange={setChosen}
        addControls={addControls}
        onAddControlsChange={setAddControls}
        importError={importPack.isError ? importPack.error : null}
        fixError={fix.isError && imported !== null ? fix.error : null}
        chooseOther={chooseOther}
        onOpenAnyway={() => {
          if (imported !== null) openPack(imported);
        }}
      />
    );
    footer = canClean ? (
      <>
        {selected.size === 0 ? (
          <p className="grow t-sm t-3">{t('abrir.higiene.nenhumMarcado')}</p>
        ) : null}
        {cancel}
        <Button
          variant="ghost"
          disabled={busy || imported !== null}
          onClick={() => {
            run(false);
          }}
        >
          {t('abrir.higiene.agoraNao')}
        </Button>
        <Button
          variant="primary"
          loading={busy}
          disabled={selected.size === 0 || imported !== null}
          onClick={() => {
            run(true);
          }}
        >
          {busy
            ? t('abrir.higiene.limpando')
            : t('abrir.higiene.limparEAbrir', { count: selected.size })}
        </Button>
      </>
    ) : (
      <>
        {cancel}
        <Button
          variant="primary"
          loading={busy}
          disabled={imported !== null}
          onClick={() => {
            run(false);
          }}
        >
          {busy ? t('abrir.abrindo') : readOnly ? t('abrir.abrirLeitura') : t('abrir.abrir')}
        </Button>
      </>
    );
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={t('abrir.titulo')}
        description={t('abrir.sub', { path })}
        returnFocusTo={returnFocusTo}
        footer={footer}
      >
        {body}
      </DialogContent>
    </Dialog>
  );
}

/** Recusa da verificação: loader não suportado, sem `pack.toml` legível ou outro erro. */
function PreviewError({ error, action }: { error: unknown; action: ReactNode }) {
  const { t } = useTranslation('packs');
  const appError = commandError(error);
  if (appError?.code.domain === 'project' && appError.code.code === 'UNSUPPORTED_LOADER') {
    const loader = appError.params.loader;
    const reason = appError.params.reason;
    let title: string;
    if (loader) title = t('abrir.quilt', { loader: loaderName(loader) });
    else if (reason === 'multiple') title = t('abrir.variosLoaders');
    else title = t('abrir.loaderDesconhecido');
    return (
      <Alert kind="danger" title={title} actions={action}>
        <div className="alert__text">{t('abrir.naoMudou')}</div>
      </Alert>
    );
  }
  if (appError?.code.domain === 'project' && appError.code.code === 'INVALID_PACK') {
    return (
      <div className="stack-3">
        <ErrorPanel error={error} title={t('abrir.semPackToml')} actions={action} />
        <p className="t-sm t-2">{t('abrir.semPackTomlTexto')}</p>
      </div>
    );
  }
  return <ErrorPanel error={error} actions={action} />;
}

function PreviewReady({
  preview,
  selected,
  onSelectedChange,
  addControls,
  onAddControlsChange,
  importError,
  fixError,
  chooseOther,
  onOpenAnyway,
}: {
  preview: ImportPreview;
  selected: ReadonlySet<string>;
  onSelectedChange: (selected: ReadonlySet<string>) => void;
  addControls: boolean;
  onAddControlsChange: (checked: boolean) => void;
  importError: unknown;
  fixError: unknown;
  chooseOther: ReactNode;
  onOpenAnyway: () => void;
}) {
  const { t } = useTranslation('packs');
  const readOnly = preview.readOnlyReason !== null;
  const canClean = !readOnly && preview.hygiene.length > 0;
  const loader = preview.loader ? loaderName(preview.loader) : t('lista.vanilla');
  return (
    <div className="stack">
      <div className="packcard">
        <NameTile seed={preview.name} size="xl" />
        <div className="packcard__name">{preview.name}</div>
        <div className="packcard__meta">
          {t('pack.sub', { mc: preview.minecraft ?? '?', loader })}
        </div>
        <dl className="packcard__facts">
          <div>
            <dt>{t('abrir.fatoLoader')}</dt>
            <dd>
              <span className="status status--ok">{t('abrir.loaderSuportado')}</span>
            </dd>
          </div>
          <div>
            <dt>{t('abrir.fatoPasta')}</dt>
            <dd className="path">{preview.path}</dd>
          </div>
        </dl>
      </div>

      {readOnly ? (
        <Alert kind="warn" title={t('abrir.soLeituraTitulo')}>
          <div className="alert__text">{t('abrir.soLeituraTexto')}</div>
          <pre className="errpanel__pre">{preview.readOnlyReason}</pre>
        </Alert>
      ) : null}

      {canClean ? (
        <HygieneTable
          items={preview.hygiene}
          selected={selected}
          onSelectedChange={onSelectedChange}
        />
      ) : null}

      {!readOnly && preview.controlDiffs.length > 0 ? (
        <ControlsPanel
          preview={preview}
          checked={addControls}
          onCheckedChange={onAddControlsChange}
        />
      ) : null}

      {importError !== null ? <ErrorPanel error={importError} actions={chooseOther} /> : null}
      {fixError !== null ? (
        <ErrorPanel
          error={fixError}
          title={t('abrir.limpezaFalhou')}
          actions={
            <Button size="sm" onClick={onOpenAnyway}>
              {t('abrir.abrirMesmoAssim')}
            </Button>
          }
        />
      ) : null}
    </div>
  );
}

function ControlsPanel({
  preview,
  checked,
  onCheckedChange,
}: {
  preview: ImportPreview;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  const { t } = useTranslation('packs');
  return (
    <section className="panel" aria-labelledby="abrir-controles">
      <h2 className="panel__title panel__title--sans" id="abrir-controles">
        {t('abrir.controles.titulo')}
      </h2>
      <p className="t-sm t-2 mt-1">{t('abrir.controles.texto')}</p>
      <label className="check mt-3">
        <input
          type="checkbox"
          checked={checked}
          onChange={(event) => {
            onCheckedChange(event.target.checked);
          }}
        />
        <span className="check__text">{t('abrir.controles.adicionar')}</span>
      </label>
      <div className="mt-3">
        {preview.controlDiffs.map((diff) => (
          <details key={diff.path} className="controls__item">
            <summary>
              <span className="t-mono">{diff.path}</span>{' '}
              <span className="tag">
                {diff.current === null
                  ? t('abrir.controles.falta')
                  : t('abrir.controles.diferente')}
              </span>
              <span className="sr-only">
                {t('abrir.controles.verDiferenca', { path: diff.path })}
              </span>
            </summary>
            <DiffView
              file={diff.path}
              before={diff.current ?? ''}
              after={diff.suggested}
              className="mt-2"
            />
          </details>
        ))}
      </div>
      <p className="field__hint mt-3">{t('abrir.controles.obrigatorio')}</p>
    </section>
  );
}
