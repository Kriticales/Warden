/**
 * O aviso da migração das chaves do `.env` antigo para o cofre (ADR-0069), uma vez por
 * abertura do app, em qualquer tela: as chaves foram (ou ainda não foram) para o cofre, com o
 * atalho para Configurações → Chaves e contas, onde o aviso completo fica.
 */
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { showToast } from '../../../components/ui/toast';
import { useKeysOverview } from '../api';
import { useVaultName } from '../layout/vault';
import { KEYS_PAGE } from '../pages';

/** O aviso já foi mostrado nesta abertura do app. */
let shown = false;

/** Só para os testes: volta ao estado de uma abertura nova. */
export function resetMigrationToastForTests(): void {
  shown = false;
}

export function MigrationToast() {
  const { t } = useTranslation('configuracoes');
  const navigate = useNavigate();
  const vault = useVaultName();
  const overview = useKeysOverview();
  const migration = overview.data?.migration ?? null;

  useEffect(() => {
    if (!migration || shown) return;
    shown = true;
    const moved = migration.kind === 'moved';
    showToast({
      kind: moved ? 'ok' : 'danger',
      title: moved
        ? t('chaves.migracao.toastMovidas', { cofre: vault })
        : t('chaves.migracao.toastFalhou'),
      text: t('chaves.migracao.toastTexto'),
      persistent: !moved,
      action: {
        label: t('chaves.migracao.abrir'),
        altText: t('chaves.migracao.toastTexto'),
        onClick: () => {
          void navigate({ to: '/configuracoes/$pagina', params: { pagina: KEYS_PAGE } });
        },
      },
    });
  }, [migration, navigate, t, vault]);

  return null;
}
