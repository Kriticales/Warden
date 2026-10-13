/**
 * Aviso do cabeçalho "Mudanças do teste para revisar" (SPEC T05 e T15; ESTRUTURA §7), registrado
 * em `pack-editor/header/slots.ts`. Aparece enquanto há mudanças do último teste esperando
 * decisão (depois de "Decidir depois", por exemplo); clicar leva ao resultado desse teste. Na
 * própria tela do teste não aparece: a revisão já está ali.
 */
import { useRouterState } from '@tanstack/react-router';
import { TriangleAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../components/ui/icon';
import type { HeaderSlotProps } from '../pack-editor/header/slots';
import { useOpenTest } from '../test/TestButton';
import { useCapture } from './api';

export function CaptureAlert({ packId }: HeaderSlotProps) {
  const { t } = useTranslation('captura');
  const capture = useCapture(packId);
  const openTest = useOpenTest();
  const onTestPage = useRouterState({
    select: (state) => state.location.pathname.endsWith('/teste'),
  });
  const view = capture.data;
  if (!view?.needsReview || onTestPage) {
    return null;
  }
  return (
    <button
      type="button"
      className="headalert headalert--warn"
      onClick={() => {
        openTest(packId, view.sessionId);
      }}
    >
      <Icon icon={TriangleAlert} size="sm" />
      {t('aviso')}
    </button>
  );
}
