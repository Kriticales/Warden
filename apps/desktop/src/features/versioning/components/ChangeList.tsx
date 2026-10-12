/**
 * As mudanças entre dois estados do pack, nos mesmos grupos do changelog (SPEC T16 e T17):
 * Minecraft e loader, mods adicionados, removidos, atualizados e ajustados, resource packs e
 * shaders e configs alteradas. Usada nas alterações não salvas e em "Ver diferenças para o
 * estado atual".
 */
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { ChangeSet, ItemChange } from '../../../lib/ipc/bindings';
import { loaderName } from '../../packs/lib/pack-list';
import { groupChanges, hasOnlyControlFiles } from '../model';

export interface ChangeListProps {
  changes: ChangeSet;
  /** Quando dado, cada config ganha o botão Descartar (P1). */
  renderConfigAction?: ((path: string) => ReactNode) | undefined;
}

export function ChangeList({ changes, renderConfigAction }: ChangeListProps) {
  const { t } = useTranslation('versoes');
  const groups = groupChanges(changes);

  if (changes.files.length === 0) {
    return <p className="t-sm t-3">{t('mudancas.semAlteracoes')}</p>;
  }

  const section = (title: string, lines: ReactNode[]) =>
    lines.length === 0 ? null : (
      <div className="changes__group">
        <div className="t-caps t-3">{title}</div>
        <ul className="changes__list">
          {lines.map((line, index) => (
            <li key={index}>{line}</li>
          ))}
        </ul>
      </div>
    );

  const platform: ReactNode[] = [];
  if (changes.minecraft) {
    platform.push(
      t('mudancas.minecraft', {
        de: changes.minecraft.old ?? '?',
        para: changes.minecraft.new ?? '?',
      }),
    );
  }
  for (const loader of changes.loaders) {
    const { old: from, new: to } = loader.change;
    const name = loaderName(loader.loader);
    if (from !== null && to !== null) {
      platform.push(t('mudancas.loaderTroca', { loader: name, de: from, para: to }));
    } else if (to !== null) {
      platform.push(t('mudancas.loaderNovo', { loader: name, para: to }));
    } else if (from !== null) {
      platform.push(t('mudancas.loaderSaiu', { loader: name, de: from }));
    }
  }

  const versioned = (item: ItemChange) => {
    const label = item.new?.label ?? item.old?.label;
    return label ? `${item.name} ${label}` : item.name;
  };
  const updated = (item: ItemChange) =>
    `${item.name} ${item.old?.label ?? '?'} → ${item.new?.label ?? '?'}`;
  const other = (item: ItemChange) => {
    const key = {
      added: 'mudancas.outroAdicionado',
      removed: 'mudancas.outroRemovido',
      updated: 'mudancas.outroAtualizado',
      adjusted: 'mudancas.outroAjustado',
    } as const;
    const name = item.kind === 'updated' ? updated(item) : item.name;
    return t(key[item.kind], { nome: name });
  };

  const configs = groups.configs.map((path) => (
    <span className="changes__config" key={path}>
      <span className="path">{path}</span>
      {renderConfigAction?.(path)}
    </span>
  ));

  const body = [
    section(t('mudancas.plataforma'), platform),
    section(t('mudancas.adicionados'), groups.added.map(versioned)),
    section(t('mudancas.removidos'), groups.removed.map(versioned)),
    section(t('mudancas.atualizados'), groups.updated.map(updated)),
    section(
      t('mudancas.ajustados'),
      groups.adjusted.map((item) => t('mudancas.ajustado', { nome: item.name })),
    ),
    section(t('mudancas.outros'), groups.others.map(other)),
    section(t('mudancas.configs'), configs),
  ].filter((node) => node !== null);

  return (
    <div className="changes">
      {body}
      {hasOnlyControlFiles(changes) ? <p className="t-sm t-3">{t('mudancas.soControle')}</p> : null}
    </div>
  );
}
