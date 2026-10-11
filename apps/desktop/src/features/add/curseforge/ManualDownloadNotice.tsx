/**
 * "Download manual necessário" (SPEC T08): o autor bloqueou downloads por apps de terceiros na
 * CurseForge. A marca da lista e o aviso da pré-visualização usam estes textos.
 */
import { Download } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Icon } from '../../../components/ui/icon';

/** A marca curta da linha de resultado. */
export function ManualDownloadTag() {
  const { t } = useTranslation('adicionarCurseforge');
  return (
    <span className="tag tag--warn" title={t('downloadManualDica')}>
      <Icon icon={Download} />
      {t('downloadManual')}
    </span>
  );
}

/** O aviso da pré-visualização, quando a fonte escolhida é a CurseForge. */
export function ManualDownloadNotice({ name }: { name: string }) {
  const { t } = useTranslation('adicionarCurseforge');
  return (
    <Alert kind="warn" compact title={t('downloadManualAviso.titulo')} role="status">
      <p>{t('downloadManualAviso.texto', { name })}</p>
    </Alert>
  );
}
