/**
 * Abrir pack existente (SPEC T04; protótipo `abrir-existente`). A pasta escolhida em "Abrir
 * ou importar…" é verificada sem escrita (`pack_import_preview`): loader suportado, estado do
 * git, higiene e arquivos de controle. Só "Abrir pack", "Agora não" ou "Limpar N arquivos e
 * abrir" escrevem: registram o pack (`pack_import`) e, com Limpar, apagam os marcados depois de
 * um ponto de segurança (`pack_hygiene_fix`).
 */
import { useNavigate } from '@tanstack/react-router';
import { FolderOpen } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { DiffView } from '../../../components/common/DiffView';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { NameTile } from '../../../components/common/PixelArt';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { ImportPreview, PackId } from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { useHygieneFix, useImportPack, useImportPreview } from '../api';
import { useOpenOrImport } from '../hooks/useOpenOrImport';
import { loaderName } from '../lib/pack-list';
import { HygieneTable } from './HygieneTable';
import '../packs.css';

export function OpenPackPage({ path }: { path: string | null }) {
  const { t } = useTranslation('packs');
  const preview = useImportPreview(path);
  const openOrImport = useOpenOrImport();
  const chooseButton = (label: string) => (
    <Button size="sm" icon={FolderOpen} loading={openOrImport.pending} onClick={openOrImport.start}>
      {label}
    </Button>
  );

  let body;
  if (path === null) {
    body = (
      <>
        <PageHead title={t('abrir.titulo')} sub={t('abrir.semPasta')} />
        {chooseButton(t('abrir.escolherPasta'))}
      </>
    );
  } else if (preview.isPending) {
    body = (
      <>
        <PageHead title={t('abrir.titulo')} sub={t('abrir.sub', { path })} />
        <div className="wizard__body--wide">
          <ProgressBar value={null} label={t('abrir.lendo')} meta={t('abrir.lendoMeta')} />
        </div>
      </>
    );
  } else if (preview.isError) {
    body = (
      <>
        <PageHead title={t('abrir.titulo')} sub={t('abrir.sub', { path })} />
        <div className="openpack">
          <PreviewError error={preview.error} action={chooseButton(t('abrir.escolherOutra'))} />
        </div>
      </>
    );
  } else {
    body = (
      <>
        <PageHead title={t('abrir.titulo')} sub={t('abrir.sub', { path })} />
        <PreviewReady preview={preview.data} chooseOther={chooseButton(t('abrir.escolherOutra'))} />
      </>
    );
  }

  return (
    <AppPage back={{ to: '/packs', label: t('pack.voltar') }} where={t('abrir.onde')}>
      {body}
    </AppPage>
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
  chooseOther,
}: {
  preview: ImportPreview;
  chooseOther: ReactNode;
}) {
  const { t } = useTranslation('packs');
  const navigate = useNavigate();
  const importPack = useImportPack();
  const fix = useHygieneFix();
  const [selected, setSelected] = useState<ReadonlySet<string>>(
    () => new Set(preview.hygiene.map((item) => item.path)),
  );
  const [addControls, setAddControls] = useState(true);
  const [imported, setImported] = useState<PackId | null>(null);
  const readOnly = preview.readOnlyReason !== null;
  const canClean = !readOnly && preview.hygiene.length > 0;
  const busy = importPack.isPending || fix.isPending;

  const openPack = (packId: PackId) => {
    void navigate({ to: '/packs/$packId', params: { packId } });
  };

  const run = (clean: boolean) => {
    const paths = clean ? [...selected] : [];
    importPack.mutate(
      { path: preview.path, addControls: addControls && !readOnly },
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
            showToast({ kind: 'ok', title: t('abrir.feito', { name: preview.name }) });
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

  const loader = preview.loader ? loaderName(preview.loader) : t('lista.vanilla');
  return (
    <div className="openpack stack">
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
        <HygieneTable items={preview.hygiene} selected={selected} onSelectedChange={setSelected} />
      ) : null}

      {!readOnly && preview.controlDiffs.length > 0 ? (
        <ControlsPanel preview={preview} checked={addControls} onCheckedChange={setAddControls} />
      ) : null}

      {importPack.isError ? <ErrorPanel error={importPack.error} actions={chooseOther} /> : null}
      {fix.isError && imported !== null ? (
        <ErrorPanel
          error={fix.error}
          title={t('abrir.limpezaFalhou')}
          actions={
            <Button
              size="sm"
              onClick={() => {
                openPack(imported);
              }}
            >
              {t('abrir.abrirMesmoAssim')}
            </Button>
          }
        />
      ) : null}

      <div className="btn-row btn-row--end openpack__actions">
        {canClean ? (
          <>
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
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                void navigate({ to: '/packs' });
              }}
            >
              {t('abrir.cancelar')}
            </Button>
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
        )}
      </div>
      {canClean && selected.size === 0 ? (
        <p className="t-sm t-3 text-right">{t('abrir.higiene.nenhumMarcado')}</p>
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
