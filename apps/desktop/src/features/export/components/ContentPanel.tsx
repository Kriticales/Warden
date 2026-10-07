/**
 * "3. O que vai no pack" (SPEC T19, pré-visualização): os arquivos que pedem atenção (alertas)
 * e a árvore com exatamente o que vai, com contagem e tamanho por pasta. Cada arquivo (e cada
 * pasta) tem "Excluir do pack", que mostra a regra do `.packwizignore` antes de gravar.
 */
import { ChevronRight, EyeOff, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { ExportPreview, PackId, PreviewFile } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { buildTree, canExclude, collectAlerts, type FileAlert, type TreeGroup } from '../model';
import { useAlertText } from './alert-text';
import { ExcludeDialog, type ExcludeTarget } from './ExcludeDialog';

type OnExclude = ((target: ExcludeTarget) => void) | null;

export function ContentPanel({
  packId,
  preview,
  readOnly,
}: {
  packId: PackId;
  preview: ExportPreview;
  readOnly: boolean;
}) {
  const { t } = useTranslation('exportar');
  const [target, setTarget] = useState<ExcludeTarget | null>(null);
  const tree = buildTree(preview);
  const alerts = collectAlerts(preview);
  const onExclude: OnExclude = readOnly ? null : setTarget;

  return (
    <>
      {alerts.length > 0 ? <AlertsPanel alerts={alerts} onExclude={onExclude} /> : null}
      <section className="panel" aria-labelledby="ex-conteudo">
        <h2 className="panel__title panel__title--sans" id="ex-conteudo">
          {t('conteudo.titulo')}
        </h2>
        <div className="export-tree mt-3">
          <div>
            {tree.control.map((file, index) => (
              <span key={file.path}>
                {index > 0 ? ' · ' : null}
                <b>{file.path}</b>
              </span>
            ))}{' '}
            <span className="t-3">{t('conteudo.controle')}</span>
          </div>
          {tree.groups.map((group) => (
            <Group key={group.folder ?? '.'} group={group} onExclude={onExclude} />
          ))}
        </div>
        <p className="t-sm mt-3">
          {t('conteudo.total', {
            arquivos: t('conteudo.arquivos', { count: preview.files.length }),
            tamanho: formatBytes(preview.bytes),
          })}{' '}
          {readOnly ? null : <span className="t-3">{t('conteudo.dica')}</span>}
        </p>
      </section>
      <ExcludeDialog
        packId={packId}
        target={target}
        onClose={() => {
          setTarget(null);
        }}
      />
    </>
  );
}

function AlertsPanel({ alerts, onExclude }: { alerts: FileAlert[]; onExclude: OnExclude }) {
  const { t } = useTranslation('exportar');
  const alertText = useAlertText();
  const files = new Set(alerts.map((item) => item.path)).size;
  return (
    <section className="panel panel--strong" aria-labelledby="ex-alertas">
      <h2 className="panel__title panel__title--sans" id="ex-alertas">
        {t('alertas.titulo', { count: files })}
      </h2>
      <ul className="export-alerts mt-2">
        {alerts.map(({ path, alert }, index) => (
          <li key={`${path}-${String(index)}`}>
            <Icon icon={TriangleAlert} className="export-alerts__icon" />
            <span className="grow">
              <span className="path">{path}</span>
              <span className="t-2 t-sm block">{alertText(alert)}</span>
            </span>
            {onExclude && canExclude(path) ? (
              <Button
                size="sm"
                icon={EyeOff}
                aria-label={t('conteudo.excluirArquivo', { path })}
                onClick={() => {
                  onExclude({ path, folder: false });
                }}
              >
                {t('excluir.confirmar')}
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}

function Group({ group, onExclude }: { group: TreeGroup; onExclude: OnExclude }) {
  const { t } = useTranslation('exportar');
  const folder = group.folder;
  const refs = t('conteudo.referencias', { count: group.references });
  const files = `${t('conteudo.arquivos', { count: group.count })}, ${formatBytes(group.bytes)}`;
  const summary =
    group.references === group.count
      ? `${refs} ${t('conteudo.referenciasNota')}`
      : group.references > 0
        ? `${files} · ${refs}`
        : files;
  return (
    <details open={folder === null}>
      <summary>
        <Icon icon={ChevronRight} size="sm" />
        <b>{folder === null ? t('conteudo.raiz') : `${folder}/`}</b>
        <span>{summary}</span>
      </summary>
      <ul className="export-files">
        {folder !== null && onExclude && canExclude(folder) ? (
          <li>
            <Button
              variant="link"
              size="sm"
              icon={EyeOff}
              onClick={() => {
                onExclude({ path: folder, folder: true });
              }}
            >
              {t('conteudo.excluirPasta', { path: `${folder}/` })}
            </Button>
          </li>
        ) : null}
        {group.files.map((file) => (
          <FileRow
            key={file.path}
            file={file}
            name={folder === null ? file.path : file.path.slice(folder.length + 1)}
            onExclude={onExclude}
          />
        ))}
      </ul>
    </details>
  );
}

function FileRow({
  file,
  name,
  onExclude,
}: {
  file: PreviewFile;
  name: string;
  onExclude: OnExclude;
}) {
  const { t } = useTranslation('exportar');
  const alertText = useAlertText();
  const warning = file.alerts.map(alertText).join(' ');
  return (
    <li>
      <span className="path grow" title={file.path}>
        {name}
      </span>
      {warning ? (
        <span className="tag tag--warn" title={warning}>
          <Icon icon={TriangleAlert} size="sm" />
          <span className="sr-only">{warning}</span>
        </span>
      ) : null}
      {file.local && !file.config ? (
        <span className="tag tag--local">
          <span className="tag__mark" />
          {t('conteudo.local')}
        </span>
      ) : null}
      {file.reference ? null : <span className="t-3">{formatBytes(file.bytes)}</span>}
      {onExclude && canExclude(file.path) ? (
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          icon={EyeOff}
          onClick={() => {
            onExclude({ path: file.path, folder: false });
          }}
        >
          {t('conteudo.excluirArquivo', { path: file.path })}
        </Button>
      ) : null}
    </li>
  );
}
