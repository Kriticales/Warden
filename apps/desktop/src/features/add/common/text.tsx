/**
 * Pedaços de texto e marcas comuns da página Adicionar: "há 3 dias", "Minecraft 1.21.1 com
 * Fabric" e a marca de fonte (`design/system/components.js` → `source`).
 */
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';

import type { SourceRef } from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';
import { ago, sourceLabel } from './model';

/** O que o pack aceita (filtros travados). */
export interface PackTarget {
  minecraft: string;
  loader: string | null;
}

/** "Minecraft 1.21.1 com Fabric" (ou só "Minecraft 1.21.1" num pack sem loader). */
export function useTargetText(target: PackTarget): string {
  const { t } = useTranslation('adicionar');
  return target.loader
    ? t('alvo', { minecraft: target.minecraft, loader: loaderName(target.loader) })
    : t('alvoSemLoader', { minecraft: target.minecraft });
}

/** "hoje", "ontem", "há 3 dias"… */
export function useAgoText(): (iso: string) => string {
  const { t } = useTranslation('adicionar');
  return (iso) => {
    const value = ago(iso, Date.now());
    if (!value) return '';
    switch (value.unit) {
      case 'hoje':
        return t('quando.hoje');
      case 'ontem':
        return t('quando.ontem');
      case 'dias':
        return t('quando.dias', { count: value.count });
      case 'meses':
        return t('quando.meses', { count: value.count });
      case 'anos':
        return t('quando.anos', { count: value.count });
    }
  };
}

/** Marca de fonte: "Modrinth", "CurseForge" ou "Modrinth e CurseForge". */
export function SourceMark({ sources }: { sources: readonly Pick<SourceRef, 'source'>[] }) {
  const { t } = useTranslation('adicionar');
  const label = sourceLabel(sources);
  if (label === 'ambas') {
    return (
      <span className="tag">
        <span className="tag__mark tag__mark--modrinth" aria-hidden="true" />
        <span className="tag__mark tag__mark--curseforge" aria-hidden="true" />
        {t('fonte.ambas')}
      </span>
    );
  }
  return (
    <span className={`tag tag--${label}`}>
      <span className="tag__mark" aria-hidden="true" />
      {t(`fonte.${label}`)}
    </span>
  );
}

/** Marca "Já no pack". */
export function InPackTag() {
  const { t } = useTranslation('adicionar');
  return (
    <span className="tag tag--primary">
      <Icon icon={Check} />
      {t('resultado.jaNoPack')}
    </span>
  );
}
