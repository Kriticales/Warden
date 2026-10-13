/**
 * "Publicar versão X" (SPEC T18): no Histórico, em cada versão final ainda não publicada, e no
 * aviso depois de salvar uma versão final (T16). Abre o fluxo do `PublishHost`.
 */
import { Globe } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../components/ui/button';
import type { PublishButtonProps } from '../versioning/publication';
import { usePublishStore } from './store';

export function PublishButton({ packId, version, size = 'sm' }: PublishButtonProps) {
  const { t } = useTranslation('publicar');
  const open = usePublishStore((store) => store.open);
  return (
    <Button
      variant="primary"
      size={size}
      icon={Globe}
      onClick={(event) => {
        open({ packId, version, opener: event.currentTarget });
      }}
    >
      {t('botao.publicar', { version })}
    </Button>
  );
}
