/**
 * Aviso de uma fonte que ficou de fora (SPEC T08): fonte fora do ar com **Tentar de novo**, ou
 * sem chave / chave recusada da CurseForge com **Abrir Configurações**. Vale para a busca e
 * para o início.
 */
import { Link } from '@tanstack/react-router';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import type { SourceWarning } from '../../../lib/ipc/bindings';

export function WarningAlert({
  warning,
  onRetry,
}: {
  warning: SourceWarning;
  onRetry: () => void;
}) {
  const { t } = useTranslation('adicionar');
  const fonte = t(`fonte.${warning.source}`);
  if (warning.reason === 'unavailable') {
    return (
      <Alert
        kind="warn"
        title={t('avisos.indisponivel', { fonte })}
        actions={
          <Button size="sm" icon={RefreshCw} onClick={onRetry}>
            {t('acoes.tentarDeNovo', { ns: 'comum' })}
          </Button>
        }
      >
        <p>{t('avisos.indisponivelTexto')}</p>
      </Alert>
    );
  }
  const missing = warning.reason === 'keyMissing';
  return (
    <Alert
      kind="info"
      title={missing ? t('avisos.semChave') : t('avisos.chaveRecusada')}
      actions={
        <Link to="/configuracoes" className={buttonVariants({ size: 'sm' })}>
          {t('avisos.abrirConfiguracoes')}
        </Link>
      }
    >
      <p>{missing ? t('avisos.semChaveTexto') : t('avisos.chaveRecusadaTexto')}</p>
    </Alert>
  );
}
