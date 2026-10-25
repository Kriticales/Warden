/**
 * A árvore da seção Configs em três grupos (esboço aprovado `design/bug-01/configs.html`,
 * opção 1): **No pack**, **Só no teste** e **Só neste computador**. Cada grupo é uma árvore de
 * pastas (as preferências pessoais, uma lista com nomes amigáveis e cadeado).
 *
 * Teclado: só um item por vez fica na ordem do Tab (o escolhido, ou o primeiro); as setas para
 * cima e para baixo andam entre os itens visíveis dos três grupos, a seta para a direita abre a
 * pasta e a para a esquerda fecha. O foco é um contorno fino por dentro do item (o contorno
 * grosso do botão era a "borda branca" do relatório de bugs nº 1).
 */
import {
  ChevronRight,
  File,
  FileCode,
  Folder,
  FolderOpen,
  Gamepad2,
  Lock,
  Map as MapIcon,
  Monitor,
  Users,
  type LucideIcon,
} from 'lucide-react';
import { Fragment, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import type { PlacedConfig } from '../../../lib/ipc/bindings';
import { buildTree, baseName, languageOf, type DirNode, type TreeNode } from '../model';
import { friendlyKey, type Groups, type View } from './model';

export interface PlacesTreeProps {
  groups: Groups;
  view: View;
  selected: string | null;
  /** Caminho do arquivo com alterações não salvas. */
  dirty: string | null;
  /** Abre todas as pastas (algum filtro ligado). */
  expandAll: boolean;
  /** Arquivos só no teste quando a vista é "Do pack" (o aviso com "Ver no teste local"). */
  onlyTestCount: number;
  onSelect: (path: string) => void;
  onShowTest: () => void;
}

/** Prefixos das chaves das pastas abertas (a mesma pasta abre separada em cada grupo). */
const PACK = 'p:';
const TEST = 't:';

/** Pastas abertas no começo: `config/` e as que levam ao arquivo escolhido. */
function initiallyOpen(selected: string | null): Set<string> {
  const open = new Set<string>([`${PACK}config`, `${TEST}config`]);
  if (selected) {
    const parts = selected.split('/').slice(0, -1);
    for (let index = 1; index <= parts.length; index += 1) {
      const path = parts.slice(0, index).join('/');
      open.add(`${PACK}${path}`);
      open.add(`${TEST}${path}`);
    }
  }
  return open;
}

/** A chave do primeiro item desenhado: a pasta (`prefixo+caminho`) ou o arquivo (caminho). */
function firstKey(groups: Groups, view: View): string | null {
  const lists: [string, PlacedConfig[]][] =
    view === 'pack'
      ? [[PACK, groups.pack]]
      : [
          [PACK, groups.pack],
          [TEST, groups.test],
        ];
  for (const [prefix, files] of lists) {
    const node = buildTree(files)[0];
    if (node) return node.kind === 'dir' ? `${prefix}${node.path}` : node.path;
  }
  return view === 'pack' ? null : (groups.personal[0]?.path ?? null);
}

function fileIcon(path: string): LucideIcon {
  const language = languageOf(path);
  return language === 'javascript' || language === 'typescript' ? FileCode : File;
}

function friendlyIcon(path: string): LucideIcon {
  const key = friendlyKey(path);
  if (key === 'opcoes' || key === 'optifine' || key === 'shaders') return Gamepad2;
  if (key === 'minimapa' || key === 'mapa') return MapIcon;
  return File;
}

/** Move o foco entre os itens visíveis (todas as árvores da coluna). */
function moveFocus(root: HTMLElement, step: 1 | -1 | 'first' | 'last') {
  const items = Array.from(root.querySelectorAll<HTMLElement>('[role="treeitem"]'));
  const current = items.indexOf(document.activeElement as HTMLElement);
  const next =
    step === 'first'
      ? 0
      : step === 'last'
        ? items.length - 1
        : Math.min(Math.max(current + step, 0), items.length - 1);
  const target = items[next];
  if (target) {
    for (const item of items) item.tabIndex = -1;
    target.tabIndex = 0;
    target.focus();
  }
}

export function PlacesTree({
  groups,
  view,
  selected,
  dirty,
  expandAll,
  onlyTestCount,
  onSelect,
  onShowTest,
}: PlacesTreeProps) {
  const { t } = useTranslation('configs');
  const [open, setOpen] = useState(() => initiallyOpen(selected));
  const root = useRef<HTMLDivElement>(null);
  const byPath = new Map<string, PlacedConfig>();
  for (const file of [...groups.pack, ...groups.test, ...groups.personal]) {
    byPath.set(file.path, file);
  }
  // O item que fica no Tab: o escolhido, se estiver visível; senão o primeiro.
  const allPaths = [...groups.pack, ...groups.test, ...groups.personal].map((file) => file.path);
  const tabStop = selected !== null && allPaths.includes(selected) ? selected : null;
  // Sem item escolhido visível, o primeiro item desenhado fica no Tab.
  const tabKey = tabStop ?? firstKey(groups, view);
  const tabFor = (key: string): 0 | -1 => (key === tabKey ? 0 : -1);

  const toggle = (key: string, force?: boolean) => {
    setOpen((current) => {
      const next = new Set(current);
      const isOpen = next.has(key);
      if (force === true || (force === undefined && !isOpen)) next.add(key);
      else next.delete(key);
      return next;
    });
  };
  const isOpen = (prefix: string, node: DirNode) =>
    expandAll ||
    open.has(`${prefix}${node.path}`) ||
    (selected?.startsWith(`${node.path}/`) ?? false);

  const marks = (file: PlacedConfig) => (
    <span className="ctree__end">
      {dirty === file.path ? (
        <span className="mk mk--dirty" title={t('arvore.naoSalvo')}>
          <span className="sr-only">{t('arvore.naoSalvo')}</span>
        </span>
      ) : null}
      {file.place === 'pack' && file.differs ? (
        <span
          className="mk mk--diff"
          title={
            view === 'pack'
              ? t('arvore.testeDifere', { count: file.differences })
              : t('arvore.difereDoPack', { count: file.differences })
          }
        >
          {file.differences}
          <span className="sr-only">
            {view === 'pack'
              ? t('arvore.testeDifere', { count: file.differences })
              : t('arvore.difereDoPack', { count: file.differences })}
          </span>
        </span>
      ) : null}
      {file.binary ? <span className="t-3">{t('arvore.binario')}</span> : null}
    </span>
  );

  const render = (prefix: string, node: TreeNode, level: number) => {
    if (node.kind === 'dir') {
      const expanded = isOpen(prefix, node);
      return (
        <Fragment key={`${prefix}${node.path}`}>
          <button
            type="button"
            role="treeitem"
            aria-level={level}
            aria-expanded={expanded}
            aria-selected={false}
            tabIndex={tabFor(`${prefix}${node.path}`)}
            className="ctree__item ctree__item--dir"
            style={{ '--lvl': level - 1 } as React.CSSProperties}
            data-dir={`${prefix}${node.path}`}
            onClick={() => {
              toggle(`${prefix}${node.path}`, !expanded);
            }}
          >
            <ChevronRight className="icon ctree__chev" aria-hidden="true" />
            {expanded ? (
              <FolderOpen className="icon" aria-hidden="true" />
            ) : (
              <Folder className="icon" aria-hidden="true" />
            )}
            <span className="ctree__name">{node.name}</span>
            <span className="ctree__end">{node.count}</span>
          </button>
          {expanded ? (
            <div role="group">{node.children.map((child) => render(prefix, child, level + 1))}</div>
          ) : null}
        </Fragment>
      );
    }
    const file = byPath.get(node.path);
    if (!file) return null;
    const Icon = fileIcon(node.path);
    return (
      <button
        key={`${prefix}${node.path}`}
        type="button"
        role="treeitem"
        aria-level={level}
        aria-selected={selected === node.path}
        tabIndex={tabFor(node.path)}
        className="ctree__item ctree__item--file"
        style={{ '--lvl': level - 1 } as React.CSSProperties}
        title={node.path}
        onClick={() => {
          onSelect(node.path);
        }}
      >
        <span aria-hidden="true" />
        <Icon className="icon" aria-hidden="true" />
        <span className="ctree__name">{node.name}</span>
        {marks(file)}
      </button>
    );
  };

  const folderTree = (prefix: string, files: PlacedConfig[], label: string) => (
    <div role="tree" aria-label={label} className="ctree" tabIndex={-1} onKeyDown={onKeyDown}>
      {buildTree(files).map((node) => render(prefix, node, 1))}
    </div>
  );

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const all = root.current;
    if (!all) return;
    const target = event.target as HTMLElement;
    if (target.getAttribute('role') !== 'treeitem') return;
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        moveFocus(all, 1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        moveFocus(all, -1);
        break;
      case 'Home':
        event.preventDefault();
        moveFocus(all, 'first');
        break;
      case 'End':
        event.preventDefault();
        moveFocus(all, 'last');
        break;
      case 'ArrowRight':
      case 'ArrowLeft': {
        const key = target.dataset.dir;
        if (key) {
          event.preventDefault();
          toggle(key, event.key === 'ArrowRight');
        }
        break;
      }
      default:
        break;
    }
  };

  return (
    <div ref={root} className="ctrees">
      {view === 'pack' ? (
        <>
          {groups.pack.length > 0 ? (
            folderTree(PACK, groups.pack, t('grupos.packArvore'))
          ) : (
            <p className="ctree__note">{t('arvore.nenhumNoPack')}</p>
          )}
          {onlyTestCount > 0 ? (
            <div className="hintbox">
              <span>
                <strong>{t('grupos.soTesteAviso', { count: onlyTestCount })}</strong>{' '}
                {t('grupos.soTesteAvisoTexto')}
              </span>
              <button type="button" className="btn btn--sm" onClick={onShowTest}>
                <Monitor className="icon icon--sm" aria-hidden="true" />
                {t('grupos.verNoTeste')}
              </button>
            </div>
          ) : null}
        </>
      ) : (
        <>
          <div className="tgroup tgroup--pack">
            <Users className="icon" aria-hidden="true" />
            {t('grupos.pack')}
            <span className="tgroup__n">{groups.pack.length}</span>
          </div>
          {groups.pack.length > 0 ? (
            folderTree(PACK, groups.pack, t('grupos.packArvore'))
          ) : (
            <p className="ctree__note">{t('grupos.vazio')}</p>
          )}
          <div className="tgroup tgroup--test">
            <Monitor className="icon" aria-hidden="true" />
            {t('grupos.teste')}
            <span className="tgroup__n">{groups.test.length}</span>
          </div>
          <p className="ctree__note">{t('grupos.testeNota')}</p>
          {groups.test.length > 0 ? folderTree(TEST, groups.test, t('grupos.testeArvore')) : null}
          <div className="tgroup tgroup--own">
            <Lock className="icon" aria-hidden="true" />
            {t('grupos.pessoal')}
            <span className="tgroup__n">{groups.personal.length}</span>
          </div>
          <p className="ctree__note">{t('grupos.pessoalNota')}</p>
          {groups.personal.length > 0 ? (
            <div
              role="tree"
              aria-label={t('grupos.pessoalArvore')}
              className="ctree"
              tabIndex={-1}
              onKeyDown={onKeyDown}
            >
              {groups.personal.map((file) => {
                const key = friendlyKey(file.path);
                const Icon = friendlyIcon(file.path);
                return (
                  <button
                    key={file.path}
                    type="button"
                    role="treeitem"
                    aria-level={1}
                    aria-selected={selected === file.path}
                    tabIndex={tabFor(file.path)}
                    className="ctree__item ctree__item--file ctree__item--friendly"
                    title={file.path}
                    onClick={() => {
                      onSelect(file.path);
                    }}
                  >
                    <span aria-hidden="true" />
                    <Icon className="icon" aria-hidden="true" />
                    <span className="ctree__name">
                      {key ? t(`pessoal.nomes.${key}`) : baseName(file.path)}
                      {key ? <span className="ctree__sub">{baseName(file.path)}</span> : null}
                    </span>
                    <span className="ctree__end">
                      {dirty === file.path ? (
                        <span className="mk mk--dirty" title={t('arvore.naoSalvo')}>
                          <span className="sr-only">{t('arvore.naoSalvo')}</span>
                        </span>
                      ) : null}
                      <span className="mk mk--own" title={t('pessoal.marca')}>
                        <Lock className="icon" aria-hidden="true" />
                        <span className="sr-only">{t('pessoal.marca')}</span>
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
