/**
 * Seção Configs do pack (SPEC T12; protótipo `configs`): à esquerda a origem dos arquivos, o
 * filtro por nome e a árvore; à direita o editor de texto do arquivo escolhido.
 *
 * O arquivo e a origem escolhidos moram na URL (`arquivo`, `origem`), então voltar do navegador,
 * trocar de seção e abrir a partir de outra tela passam todos pelo mesmo caminho. Com alterações
 * não salvas, qualquer saída (outro arquivo, outra origem, outra seção, fechar o app) pede
 * confirmação: Salvar e continuar, Descartar ou Continuar editando.
 */
import { useBlocker } from '@tanstack/react-router';
import { Search, X } from 'lucide-react';
import { useCallback, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { EmptyState } from '../../components/common/EmptyState';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
} from '../../components/ui/alert-dialog';
import { Button } from '../../components/ui/button';
import type { ConfigOrigin, PackId } from '../../lib/ipc/bindings';
import { usePack } from '../packs/api';
import { useSettings } from '../settings/api';
import { useConfigFile, useConfigTree } from './api';
import { ConfigEditor, type EditorGuard } from './components/ConfigEditor';
import { FileTree } from './components/FileTree';
import { buildTree, filterFiles } from './model';
import './configs.css';

const ORIGINS = ['pack', 'instance'] as const satisfies readonly ConfigOrigin[];

export interface ConfigsSearch {
  /** `instancia` mostra os arquivos da instância de teste; sem isso, os do pack. */
  origem?: 'instancia';
  /** Caminho do arquivo aberto. */
  arquivo?: string;
}

export interface ConfigsPageProps {
  packId: PackId;
  search: ConfigsSearch;
  onNavigate: (search: ConfigsSearch) => void;
}

export function ConfigsPage({ packId, search, onNavigate }: ConfigsPageProps) {
  const { t } = useTranslation('configs');
  const origin: ConfigOrigin = search.origem === 'instancia' ? 'instance' : 'pack';
  const path = search.arquivo ?? null;
  const pack = usePack(packId);
  const settings = useSettings();
  const tree = useConfigTree(packId, origin);
  const file = useConfigFile(packId, origin, path);
  const [filter, setFilter] = useState('');

  // As alterações não salvas do editor aberto; a referência é para a guarda ler o valor atual.
  const guard = useRef<EditorGuard | null>(null);
  const [dirty, setDirty] = useState(false);
  const onGuard = useCallback((next: EditorGuard | null) => {
    guard.current = next;
    setDirty(next?.dirty ?? false);
  }, []);

  const blocker = useBlocker({
    shouldBlockFn: () => guard.current?.dirty ?? false,
    enableBeforeUnload: () => guard.current?.dirty ?? false,
    withResolver: true,
  });

  const nodes = useMemo(
    () => buildTree(filterFiles(tree.data?.files ?? [], filter)),
    [tree.data, filter],
  );
  const visible = nodes.length > 0;
  const filtering = filter.trim() !== '';

  const select = (nextPath: string) => {
    onNavigate({
      ...(origin === 'instance' ? { origem: 'instancia' as const } : {}),
      arquivo: nextPath,
    });
  };

  return (
    <div className="stack cfg-page">
      <PageHead title={t('titulo')} sub={t('sub')} />
      {origin === 'instance' ? (
        <Alert kind="warn" title={t('instancia.aviso')}>
          {t('instancia.avisoTexto')}
        </Alert>
      ) : null}
      <div className="cfg-layout">
        <div className="cfg-layout__side">
          <select
            className="select select--sm"
            aria-label={t('origem.rotulo')}
            value={origin}
            onChange={(event) => {
              onNavigate(event.target.value === 'instance' ? { origem: 'instancia' } : {});
              setFilter('');
            }}
          >
            {ORIGINS.map((value) => (
              <option key={value} value={value}>
                {t(`origem.${value}`)}
              </option>
            ))}
          </select>
          <div className="cfg-filter">
            <Search className="icon icon--sm" aria-hidden="true" />
            <input
              className="input input--sm"
              type="search"
              value={filter}
              placeholder={t('arvore.filtro')}
              aria-label={t('arvore.filtro')}
              onChange={(event) => {
                setFilter(event.target.value);
              }}
            />
            {filtering ? (
              <Button
                size="sm"
                variant="ghost"
                iconOnly
                icon={X}
                onClick={() => {
                  setFilter('');
                }}
              >
                {t('arvore.filtroLimpar')}
              </Button>
            ) : null}
          </div>
          {tree.isPending ? (
            <LoadingState label={t('carregando')} />
          ) : tree.isError ? (
            <ErrorPanel
              title={t('erroArvore')}
              error={tree.error}
              onRetry={() => {
                void tree.refetch();
              }}
            />
          ) : (
            <>
              {tree.data.truncated ? (
                <p className="t-xs t-3">{t('arvore.cortada', { max: '20.000' })}</p>
              ) : null}
              {visible ? (
                <FileTree
                  key={`${origin}:${filtering ? 'filtro' : 'tudo'}`}
                  nodes={nodes}
                  selected={path}
                  dirty={dirty ? path : null}
                  expandAll={filtering}
                  onSelect={select}
                />
              ) : tree.data.available && filtering ? (
                <p className="t-sm t-3">{t('arvore.semResultado', { busca: filter.trim() })}</p>
              ) : null}
            </>
          )}
        </div>

        <div className="cfg-layout__main">
          {tree.isSuccess && !tree.data.available ? (
            <EmptyState
              glyph="dots"
              title={t(origin === 'instance' ? 'vazio.instanciaTitulo' : 'vazio.titulo')}
              text={t(origin === 'instance' ? 'vazio.instanciaTexto' : 'vazio.texto')}
            />
          ) : tree.isSuccess && tree.data.files.length === 0 ? (
            <EmptyState
              glyph="dots"
              title={t(origin === 'instance' ? 'vazio.instanciaSemArquivos' : 'vazio.titulo')}
              text={origin === 'instance' ? undefined : t('vazio.texto')}
            />
          ) : path === null ? (
            <EmptyState
              compact
              glyph="box"
              title={t('vazio.semArquivoTitulo')}
              text={t('vazio.semArquivoTexto')}
            />
          ) : file.isPending ? (
            <LoadingState label={t('editor.carregandoArquivo')} />
          ) : file.isError ? (
            <ErrorPanel
              title={t('editor.erroArquivo', { arquivo: path })}
              error={file.error}
              onRetry={() => {
                void file.refetch();
              }}
            />
          ) : (
            <ConfigEditor
              key={`${origin}:${file.data.path}`}
              packId={packId}
              origin={origin}
              file={file.data}
              loader={pack.data?.loader ?? null}
              minecraft={pack.data?.minecraft ?? null}
              showDiff={settings.data?.configDiffBeforeSave ?? true}
              onGuard={onGuard}
            />
          )}
        </div>
      </div>

      <AlertDialog
        open={blocker.status === 'blocked'}
        onOpenChange={(open) => {
          if (!open) blocker.reset?.();
        }}
      >
        <AlertDialogContent
          title={t('sair.titulo')}
          description={t('sair.texto', { arquivo: path ?? '' })}
          footer={
            <>
              <AlertDialogCancel asChild>
                <Button variant="ghost">{t('sair.ficar')}</Button>
              </AlertDialogCancel>
              <Button
                variant="danger"
                onClick={() => {
                  blocker.proceed?.();
                }}
              >
                {t('sair.descartar')}
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  void (guard.current?.save() ?? Promise.resolve(true)).then((saved) => {
                    if (saved) blocker.proceed?.();
                    else blocker.reset?.();
                  });
                }}
              >
                {t('sair.salvar')}
              </Button>
            </>
          }
        />
      </AlertDialog>
    </div>
  );
}
