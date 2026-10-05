/**
 * Barra do nível do app (DESIGN-SYSTEM §5; ESTRUTURA): marca, onde você está e os destinos de
 * `navigation.ts` (Configurações). Sem menu lateral. Veia e camada de sculk só aqui e no
 * cabeçalho do pack (`components.css`).
 */
import { Link, type LinkProps } from '@tanstack/react-router';
import { ArrowLeft } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { BrandMark } from '../../components/common/PixelArt';
import { buttonVariants } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { appBarLinks } from '../navigation';

export interface TopBarProps {
  /** "← Voltar": rota e texto (por exemplo, Meus packs). */
  back?: { to: NonNullable<LinkProps['to']>; label: ReactNode };
  /** Onde você está ("Primeira execução", "Configurações"). */
  where?: ReactNode;
  /** Esconde os destinos da barra (na própria página de destino ou na primeira execução). */
  hideLinks?: boolean;
}

export function TopBar({ back, where, hideLinks = false }: TopBarProps) {
  const { t } = useTranslation('navegacao');
  return (
    <header className="topbar">
      {back ? (
        <>
          <Link to={back.to} className={buttonVariants({ variant: 'ghost', size: 'sm' })}>
            <Icon icon={ArrowLeft} />
            <span>{back.label}</span>
          </Link>
          <span className="topbar__sep" aria-hidden="true" />
        </>
      ) : null}
      <span className="brand">
        <BrandMark />
        <span className="brand__name">{t('marca')}</span>
      </span>
      {where ? (
        <>
          <span className="topbar__sep" aria-hidden="true" />
          <span className="topbar__where">{where}</span>
        </>
      ) : null}
      <span className="topbar__end">
        {hideLinks
          ? null
          : appBarLinks.map((link) => (
              <Link key={link.to} to={link.to} className={buttonVariants({ variant: 'ghost' })}>
                <Icon icon={link.icon} />
                <span>{t(link.labelKey)}</span>
              </Link>
            ))}
      </span>
    </header>
  );
}
