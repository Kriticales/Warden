/**
 * Versão instalada e arquivo (SPEC T07): número, versões do Minecraft e loaders, data, nome do
 * arquivo, tamanho e hash. Nome, arquivo e hash vêm do `.pw.toml`, então aparecem mesmo sem
 * internet (CA-T07-02).
 */
import { Copy } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../../components/ui/button';
import { formatBytes, formatDate } from '../../../../lib/format';
import { log } from '../../../../lib/log';
import type { DetailBlockProps } from '../blocks';

/** Hash longo abreviado para caber numa linha: os 8 primeiros e os 8 últimos caracteres. */
export function shortHash(hash: string): string {
  return hash.length > 20 ? `${hash.slice(0, 8)}…${hash.slice(-8)}` : hash;
}

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
          <dd>
            <span className="path">{fileName}</span>
            {size ? (
              <>
                {' '}
                <span className="t-xs t-3 itemsize">· {formatBytes(size)}</span>
              </>
            ) : null}
          </dd>
        </>
      ) : null}
      {file?.hash ? (
        <>
          <dt>{t('detalhes.hash', { format: file.hashFormat })}</dt>
          <dd>
            <HashLine hash={file.hash} />
          </dd>
        </>
      ) : null}
      <dt>{t('detalhes.caminho')}</dt>
      <dd className="path">{item.path}</dd>
    </dl>
  );
}

/** O hash numa linha só: abreviado, com o completo no `title` e um botão para copiar. */
function HashLine({ hash }: { hash: string }) {
  const { t } = useTranslation('editor');
  const [copy, setCopy] = useState<'idle' | 'copied' | 'failed'>('idle');
  const copyHash = async () => {
    try {
      await navigator.clipboard.writeText(hash);
      setCopy('copied');
    } catch (cause) {
      log.warn('falha ao copiar o hash', cause);
      setCopy('failed');
    }
  };
  return (
    <span className="itemhash">
      <span className="path" title={hash}>
        {shortHash(hash)}
      </span>
      <Button
        variant="link"
        size="sm"
        icon={Copy}
        onClick={() => {
          void copyHash();
        }}
      >
        {copy === 'copied' ? t('detalhes.hashCopiado') : t('detalhes.copiar')}
      </Button>
      {copy === 'failed' ? (
        <span className="t-xs t-danger" role="status">
          {t('detalhes.naoCopiou')}
        </span>
      ) : null}
    </span>
  );
}
