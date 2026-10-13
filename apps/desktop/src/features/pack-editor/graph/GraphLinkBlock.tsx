/**
 * "Ver no grafo" nos detalhes do item (SPEC T07, P1; D-11): abre Mods no modo Grafo com este
 * mod no centro. Entra no painel pelo registro `details/blocks.ts` (logo depois do bloco de
 * dependências), sem mexer no componente dos detalhes.
 */
import { Link } from '@tanstack/react-router';
import { Network } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { DetailBlockProps } from '../details/blocks';

export function GraphLinkBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('grafoDesenho');
  // Só mods legíveis entram no grafo.
  if (item.kind !== 'mod' || item.state !== 'ok') return null;
  return (
    <p>
      <Link
        to="/packs/$packId/mods"
        params={{ packId }}
        search={{ ver: 'grafo', centro: item.path }}
        className={buttonVariants({ variant: 'link' })}
      >
        <Icon icon={Network} size="sm" />
        <span>{t('verNoGrafo')}</span>
      </Link>
    </p>
  );
}
