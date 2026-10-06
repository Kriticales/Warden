/**
 * Cabeçalho fixo do pack (SPEC T05; `design/system/components.js` → `packHeader`): "← Meus
 * packs", o nome com "Editar informações" (T11), Minecraft, loader e versão do pack, os avisos
 * passageiros e os botões registrados em `slots.ts` (Salvar versão, ▶ Testar ▾…).
 */
import { Link } from '@tanstack/react-router';
import { ArrowLeft, Pencil } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../components/common/PixelArt';
import { Button, buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { PackRow } from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';
import { PackInfoDialog } from '../info/PackInfoDialog';
import { slotsFor } from './slots';

export function PackHeader({ pack }: { pack: PackRow }) {
  const { t } = useTranslation('editor');
  const [infoOpen, setInfoOpen] = useState(false);
  const actions = slotsFor('actions');
  const alerts = slotsFor('alerts');
  const ready = pack.status === 'ready';
  return (
    <header className="packhead" aria-label={t('cabecalho.rotulo')}>
      <div className="packhead__row">
        <Link to="/packs" className={buttonVariants({ variant: 'ghost', size: 'sm' })}>
          <Icon icon={ArrowLeft} />
          <span>{t('cabecalho.voltar')}</span>
        </Link>
        <span className="packhead__sep" aria-hidden="true" />
        <div className="packhead__id">
          <NameTile seed={pack.name} size="lg" />
          <div className="packhead__titles">
            <div className="packhead__name">
              <h1>{pack.name}</h1>
              {ready ? (
                <Button
                  variant="ghost"
                  size="sm"
                  iconOnly
                  icon={Pencil}
                  onClick={() => {
                    setInfoOpen(true);
                  }}
                >
                  {t('cabecalho.editarInformacoes')}
                </Button>
              ) : null}
            </div>
            {ready ? <PackMetaLine pack={pack} /> : null}
          </div>
        </div>
        {ready ? (
          <div className="packhead__actions">
            {actions.map(({ id, component: Slot }) => (
              <Slot key={id} packId={pack.id} pack={pack} />
            ))}
          </div>
        ) : null}
      </div>
      {ready ? (
        <div className="packhead__alerts" role="status">
          {alerts.map(({ id, component: Slot }) => (
            <Slot key={id} packId={pack.id} pack={pack} />
          ))}
        </div>
      ) : null}
      {ready ? <PackInfoDialog pack={pack} open={infoOpen} onOpenChange={setInfoOpen} /> : null}
    </header>
  );
}

/** "Minecraft 1.20.1 · Forge 47.3.0 · versão 1.4.2". */
function PackMetaLine({ pack }: { pack: PackRow }) {
  const { t } = useTranslation('editor');
  const loader = pack.loader
    ? t('cabecalho.loader', {
        loader: loaderName(pack.loader),
        version: pack.loaderVersion ?? '',
      }).trim()
    : t('cabecalho.vanilla');
  return (
    <div className="packhead__meta">
      <span>{t('cabecalho.minecraft', { version: pack.minecraft ?? '?' })}</span>
      <span className="dot" aria-hidden="true" />
      <span>{loader}</span>
      <span className="dot" aria-hidden="true" />
      <span>
        {pack.version ? t('cabecalho.versao', { version: pack.version }) : t('cabecalho.semVersao')}
      </span>
    </div>
  );
}
