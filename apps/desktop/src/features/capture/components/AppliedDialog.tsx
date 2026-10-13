/**
 * Depois de "Trazer selecionados para o pack" (SPEC T15 "Ações"; protótipo `trazido`): onde as
 * mudanças foram parar ("aparecem em Alterações não salvas"), com "Continuar editando" e
 * "Salvar versão".
 */
import { Save } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import type { PackId } from '../../../lib/ipc/bindings';
import { SaveVersionFlow } from '../../versioning/components/SaveVersionDialog';

export interface AppliedDialogProps {
  packId: PackId;
  /** Quantas mudanças foram para o pack (`null`: fechado). */
  count: number | null;
  onClose: () => void;
}

export function AppliedDialog({ packId, count, onClose }: AppliedDialogProps) {
  const { t } = useTranslation('captura');
  const [saving, setSaving] = useState(false);
  return (
    <>
      <Dialog
        open={count !== null}
        onOpenChange={(open) => {
          if (!open) onClose();
        }}
      >
        {count !== null ? (
          <DialogContent
            size="sm"
            title={t('trazido.titulo', { count })}
            footer={
              <>
                <Button variant="ghost" onClick={onClose}>
                  {t('trazido.continuar')}
                </Button>
                <Button
                  variant="primary"
                  icon={Save}
                  onClick={() => {
                    onClose();
                    setSaving(true);
                  }}
                >
                  {t('trazido.salvar')}
                </Button>
              </>
            }
          >
            <p>{t('trazido.texto')}</p>
          </DialogContent>
        ) : null}
      </Dialog>
      <SaveVersionFlow packId={packId} open={saving} onOpenChange={setSaving} />
    </>
  );
}
