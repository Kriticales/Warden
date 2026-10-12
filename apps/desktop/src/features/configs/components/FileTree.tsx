/**
 * Árvore de arquivos (SPEC T12; protótipo `.tree`): pastas que abrem e fecham, um arquivo
 * selecionado, o ponto de "alterações não salvas" e os arquivos do jogo (`options.txt`) com
 * nome amigável. Só desenha as filhas das pastas abertas, para listas de milhares de arquivos.
 */
import { ChevronRight, File, FileText, Folder, FolderOpen, Gamepad2, Lock } from 'lucide-react';
import { Fragment, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import { isGameOptions, type DirNode, type TreeNode } from '../model';

export interface FileTreeProps {
  nodes: readonly TreeNode[];
  selected: string | null;
  /** Caminho do arquivo com alterações não salvas. */
  dirty: string | null;
  /** Abre tudo (filtro ativo). */
  expandAll: boolean;
  onSelect: (path: string) => void;
}

/** Pastas abertas no começo: a de `config/` e as que levam ao arquivo escolhido. */
function initiallyOpen(nodes: readonly TreeNode[], selected: string | null): Set<string> {
  const open = new Set<string>();
  for (const node of nodes) {
    if (node.kind === 'dir' && node.path.toLowerCase() === 'config') {
      open.add(node.path);
    }
  }
  if (selected) {
    const parts = selected.split('/').slice(0, -1);
    for (let index = 1; index <= parts.length; index += 1) {
      open.add(parts.slice(0, index).join('/'));
    }
  }
  return open;
}

export function FileTree({ nodes, selected, dirty, expandAll, onSelect }: FileTreeProps) {
  const { t } = useTranslation('configs');
  const [open, setOpen] = useState(() => initiallyOpen(nodes, selected));
  const toggle = (path: string) => {
    setOpen((current) => {
      const next = new Set(current);
      if (!next.delete(path)) {
        next.add(path);
      }
      return next;
    });
  };
  // Pastas unidas na árvore ("saves/Mundo/serverconfig") abrem se qualquer parte do caminho abre.
  const isOpen = (node: DirNode) =>
    expandAll || open.has(node.path) || (selected?.startsWith(`${node.path}/`) ?? false);

  const render = (node: TreeNode, level: number) => {
    if (node.kind === 'dir') {
      const expanded = isOpen(node);
      return (
        <Fragment key={node.path}>
          <button
            type="button"
            role="treeitem"
            aria-level={level}
            aria-expanded={expanded}
            aria-selected={false}
            className="tree__item"
            onClick={() => {
              toggle(node.path);
            }}
          >
            <Icon icon={expanded ? FolderOpen : Folder} />
            <span className="cfg-tree__name">{node.name}/</span>
            <span className="t-3">({node.count})</span>
            <Icon icon={ChevronRight} className={cn('cfg-tree__chevron', expanded && 'is-open')} />
          </button>
          {expanded ? (
            <div role="group" className="cfg-tree__group">
              {node.children.map((child) => render(child, level + 1))}
            </div>
          ) : null}
        </Fragment>
      );
    }
    const game = isGameOptions(node.path);
    const isDirty = dirty === node.path;
    const binary = node.file.binary;
    const label = game ? t('arvore.opcoes', { nome: node.name }) : node.name;
    return (
      <button
        key={node.path}
        type="button"
        role="treeitem"
        aria-level={level}
        aria-selected={selected === node.path}
        className={cn('tree__item', game && 'tree__item--friendly', isDirty && 'tree__item--dirty')}
        title={node.path}
        onClick={() => {
          onSelect(node.path);
        }}
      >
        <Icon icon={binary ? Lock : game ? Gamepad2 : node.name.includes('.') ? FileText : File} />
        <span className="cfg-tree__name">{label}</span>
        {binary ? <span className="t-3">{t('arvore.binario')}</span> : null}
        {isDirty ? <span className="sr-only">{t('arvore.naoSalvo')}</span> : null}
      </button>
    );
  };

  return (
    <div role="tree" aria-label={t('arvore.rotulo')} className="tree cfg-tree">
      {nodes.map((node) => render(node, 1))}
    </div>
  );
}
