/**
 * De onde vieram os detalhes (SPEC T07; CA-T07-02): sem internet, a CurseForge mostra
 * "Detalhes indisponíveis sem internet" (o resto vem do `.pw.toml`); o Modrinth abre do cache e
 * avisa; sem a chave da CurseForge, diz onde digitá-la. Arquivo inválido: o erro do arquivo.
 */
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../../components/ui/alert';
import type { DetailBlockProps } from '../blocks';

export function OriginBlock({ item, details }: DetailBlockProps) {
  const { t } = useTranslation('editor');
  if (item.state === 'invalid') {
    return (
      <Alert kind="danger" compact title={t('detalhes.erroArquivo')}>
        <pre className="errpanel__pre">{item.error ?? ''}</pre>
      </Alert>
    );
  }
  switch (details?.source) {
    case 'offline':
      return (
        <Alert kind="warn" compact title={t('detalhes.indisponivelOffline')}>
          <div className="alert__text">{t('detalhes.indisponivelOfflineTexto')}</div>
        </Alert>
      );
    case 'noKey':
      return (
        <Alert kind="info" compact title={t('detalhes.semChave')}>
          <div className="alert__text">{t('detalhes.semChaveTexto')}</div>
        </Alert>
      );
    case 'unavailable':
      return (
        <Alert kind="warn" compact title={t('detalhes.indisponivelFonte')}>
          <div className="alert__text">{t('detalhes.indisponivelFonteTexto')}</div>
        </Alert>
      );
    case 'cache':
      return <p className="t-xs t-3">{t('detalhes.doCache')}</p>;
    case 'none':
      return (
        <p className="t-xs t-3">
          {item.source === 'url' ? t('detalhes.linkDireto') : t('detalhes.arquivoLocal')}
        </p>
      );
    case 'live':
    case undefined:
      return null;
  }
}
