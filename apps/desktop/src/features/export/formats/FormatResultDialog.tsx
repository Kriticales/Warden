/** "Arquivo exportado": tamanho, destino, trocas feitas e o que a conferência viu. */
import { FolderOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { DialogClose, DialogContent } from '../../../components/ui/dialog';
import type { FormatExportResult } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { useRevealExport } from '../api';

export function FormatResultDialog({ result }: { result: FormatExportResult }) {
  const { t } = useTranslation('exportarFormatos');
  const reveal = useRevealExport();
  const { validation } = result;
  const details = [
    t('resultado.detalhes.entradas', { count: validation.entries }),
    t('resultado.detalhes.referencias', { count: validation.references }),
    t('resultado.detalhes.overrides', { count: validation.overrides }),
    t('resultado.detalhes.jars', { count: validation.embeddedJars.length }),
  ];
  return (
    <DialogContent
      size="sm"
      title={t('resultado.titulo')}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('resultado.fechar')}</Button>
          </DialogClose>
          <Button
            variant="primary"
            icon={FolderOpen}
            loading={reveal.isPending}
            onClick={() => {
              reveal.mutate(result.path);
            }}
          >
            {t('resultado.mostrarArquivo')}
          </Button>
        </>
      }
    >
      <p>
        {t('resultado.resumo', { tamanho: formatBytes(result.bytes) })}{' '}
        <span className="path">{result.path}</span>.
      </p>
      {result.swapped > 0 ? (
        <p className="t-sm mt-2">{t('resultado.trocados', { count: result.swapped })}</p>
      ) : null}
      <Alert kind="ok" compact title={t('resultado.conferido')} className="mt-3">
        {t('resultado.conferidoTexto')} {details.join(' · ')}.
      </Alert>
      {reveal.isError ? <ErrorPanel compact error={reveal.error} className="mt-3" /> : null}
    </DialogContent>
  );
}
