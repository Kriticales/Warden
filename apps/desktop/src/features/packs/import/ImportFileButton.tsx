/**
 * "Importar arquivo…" de Meus packs (SPEC T24, "Onde"): o diálogo nativo de arquivo escolhe um
 * `.mrpack` ou zip (o diálogo de pasta do "Abrir ou importar…" não escolhe arquivos) e leva à
 * página Importar modpack, que reconhece o formato pelo conteúdo.
 */
import { FileArchive } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import { useImportFile } from './api';

export function ImportFileButton() {
  const { t } = useTranslation('importar');
  const importFile = useImportFile();
  return (
    <Tooltip content={t('entrada.dica')}>
      <Button icon={FileArchive} loading={importFile.pending} onClick={importFile.start}>
        {importFile.pending ? t('entrada.escolhendo') : t('entrada.importarArquivo')}
      </Button>
    </Tooltip>
  );
}
