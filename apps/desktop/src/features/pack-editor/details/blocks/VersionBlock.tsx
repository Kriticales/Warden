/**
 * Versão instalada e arquivo (SPEC T07): número, versões do Minecraft e loaders, data, nome do
 * arquivo, tamanho e hash. Nome, arquivo e hash vêm do `.pw.toml`, então aparecem mesmo sem
 * internet (CA-T07-02).
 */
import { useTranslation } from 'react-i18next';

import { formatBytes, formatDate } from '../../../../lib/format';
import type { DetailBlockProps } from '../blocks';

export function VersionBlock({ item, details }: DetailBlockProps) {
  const { t } = useTranslation('editor');
  if (item.state === 'invalid') {
    return (
      <dl className="kv">
        <dt>{t('detalhes.caminho')}</dt>
        <dd className="path">{item.path}</dd>
      </dl>
    );
  }
  const version = details?.version;
  const file = details?.file;
  const number = version?.number ?? item.version;
  const fileName = file?.fileName ?? version?.fileName ?? item.fileName;
  const published = version?.published ? Date.parse(version.published) : Number.NaN;
  const size = version?.sizeBytes;
  return (
    <dl className="kv">
      {number ? (
        <>
          <dt>{t('detalhes.versaoInstalada')}</dt>
          <dd className="t-mono">{number}</dd>
        </>
      ) : null}
      {version && (version.gameVersions.length > 0 || version.loaders.length > 0) ? (
        <>
          <dt>{t('detalhes.para')}</dt>
          <dd>
            {t('detalhes.paraTexto', {
              versions: version.gameVersions.join(', '),
              loaders: version.loaders.join(', '),
            })}
          </dd>
        </>
      ) : null}
      {Number.isFinite(published) ? (
        <>
          <dt>{t('detalhes.publicadaEm')}</dt>
          <dd>{formatDate(published)}</dd>
        </>
      ) : null}
      {fileName ? (
        <>
          <dt>{t('detalhes.arquivo')}</dt>
          <dd className="path">
            {size
              ? t('detalhes.arquivoTamanho', { file: fileName, size: formatBytes(size) })
              : fileName}
          </dd>
        </>
      ) : null}
      {file?.hash ? (
        <>
          <dt>{t('detalhes.hash', { format: file.hashFormat })}</dt>
          <dd className="path itemhash">{file.hash}</dd>
        </>
      ) : null}
      <dt>{t('detalhes.caminho')}</dt>
      <dd className="path">{item.path}</dd>
    </dl>
  );
}
