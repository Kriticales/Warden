/**
 * "Ver diferença de texto" (SPEC T15 "Cada arquivo"): o arquivo no início do teste e quando o
 * jogo fechou, linha a linha (`DiffView`).
 */
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../components/common/DiffView';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import type { PackId } from '../../../lib/ipc/bindings';
import { useCaptureTexts } from '../api';

export interface TextDiffDialogProps {
  packId: PackId;
  sessionId: string;
  path: string;
  onClose: () => void;
}

export function TextDiffDialog({ packId, sessionId, path, onClose }: TextDiffDialogProps) {
  const { t } = useTranslation('captura');
  const texts = useCaptureTexts(packId, sessionId, path);
  const data = texts.data;
  const hasText = data ? data.base !== null || data.instance !== null : false;
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={t('diferenca.titulo', { path })}
        description={t('diferenca.descricao')}
        footer={
          <Button variant="ghost" onClick={onClose}>
            {t('diferenca.fechar')}
          </Button>
        }
      >
        {texts.isPending ? (
          <LoadingState inline label={t('carregando')} />
        ) : texts.isError ? (
          <ErrorPanel compact error={texts.error} />
        ) : hasText && data ? (
          <DiffView file={path} before={data.base ?? ''} after={data.instance ?? ''} />
        ) : (
          <p className="t-sm t-2">{t('diferenca.semTexto')}</p>
        )}
      </DialogContent>
    </Dialog>
  );
}
