/**
 * Link que o Warden não aceita (SPEC T08 "Link"; CA-T08-07): `http://` é recusado sem baixar
 * nada; outro link desconhecido diz quais links funcionam.
 */
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';

export function RefusedLink({ kind }: { kind: 'insecure' | 'unsupported' }) {
  const { t } = useTranslation('adicionarArquivo');
  const insecure = kind === 'insecure';
  return (
    <Alert
      kind={insecure ? 'danger' : 'warn'}
      title={insecure ? t('link.recusadoHttp') : t('link.naoSuportado')}
    >
      <p>{insecure ? t('link.recusadoHttpTexto') : t('link.naoSuportadoTexto')}</p>
    </Alert>
  );
}
