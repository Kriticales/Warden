/**
 * O menu lateral de Configurações: as 8 páginas, a atual marcada (`aria-current="page"`), um
 * dado curto onde o esboço mostra (chaves recusadas no último teste, o provedor das conversas
 * novas do Sculk, o espaço usado e a versão) e, no rodapé, onde as chaves ficam (ADR-0069).
 */
import { Link } from '@tanstack/react-router';
import { ShieldCheck } from 'lucide-react';
import { Trans, useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import { Tooltip } from '../../../components/ui/tooltip';
import { cn } from '../../../lib/cn';
import { formatBytes } from '../../../lib/format';
import { useAppInfo } from '../../about/hooks/useAppInfo';
import { useAiSettings } from '../../diagnostics/ai/api';
import { providerName } from '../ai/providers';
import { useKeysOverview } from '../api';
import { firstUsable, refusedCount } from '../keys/model';
import { SETTINGS_PAGE_ICONS, SETTINGS_PAGE_IDS, type SettingsPageId } from '../pages';
import { useStorageUsage } from '../storage/api';
import { useVaultName } from './vault';

export function SettingsNav() {
  const { t } = useTranslation('configuracoes');
  const vault = useVaultName();
  return (
    <nav className="sidenav" aria-label={t('menu.rotulo')}>
      <div className="sidenav__list">
        {SETTINGS_PAGE_IDS.map((id) => (
          <Link
            key={id}
            to="/configuracoes/$pagina"
            params={{ pagina: id }}
            className="sidenav__item"
            activeProps={{ 'aria-current': 'page' }}
          >
            <Icon
              icon={SETTINGS_PAGE_ICONS[id]}
              className={cn(id === 'sensor-sculk' && 'icon--ai')}
            />
            <span className="sidenav__name">{t(`menu.paginas.${id}`)}</span>
            <NavMeta page={id} />
          </Link>
        ))}
      </div>
      <div className="sidenav__foot">
        <Icon icon={ShieldCheck} />
        <span>
          <Trans t={t} i18nKey="menu.cofre" values={{ cofre: vault }} components={{ b: <b /> }} />
        </span>
      </div>
    </nav>
  );
}

function NavMeta({ page }: { page: SettingsPageId }) {
  switch (page) {
    case 'chaves':
      return <RefusedCount />;
    case 'sensor-sculk':
      return <SculkProvider />;
    case 'armazenamento':
      return <StorageTotal />;
    case 'sobre':
      return <AppVersion />;
    case 'geral':
    case 'teste':
    case 'editor':
    case 'registros':
      return null;
  }
}

function RefusedCount() {
  const { t } = useTranslation('configuracoes');
  const overview = useKeysOverview();
  const count = refusedCount(overview.data);
  if (count === 0) return null;
  const label = t('menu.recusadas', { count });
  return (
    <Tooltip content={label}>
      <span className="count count--danger" aria-label={label}>
        {count}
      </span>
    </Tooltip>
  );
}

function SculkProvider() {
  const { t } = useTranslation('configuracoes');
  const settings = useAiSettings();
  if (!settings.data) return null;
  const provider = firstUsable(settings.data);
  return (
    <span className="sidenav__meta">{provider ? providerName(provider) : t('menu.semChave')}</span>
  );
}

function StorageTotal() {
  const usage = useStorageUsage();
  if (!usage.data) return null;
  const { cache, minecraft, javas, instances } = usage.data;
  return (
    <span className="sidenav__meta">{formatBytes(cache + minecraft + javas + instances)}</span>
  );
}

function AppVersion() {
  const info = useAppInfo();
  if (!info.data) return null;
  return <span className="sidenav__meta t-mono">{info.data.version}</span>;
}
