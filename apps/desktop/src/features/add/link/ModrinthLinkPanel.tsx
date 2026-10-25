/**
 * Link do Modrinth colado no campo único (SPEC T08 "Link"; P1-11): a tela lê o link pela API
 * do Modrinth e segue pelo caminho normal.
 *
 * - **Projeto:** abre a pré-visualização com o seletor de versão, igual à busca.
 * - **Versão:** abre o diálogo de dependências com aquela versão exata, que avisa se ela não
 *   serve para o pack ou não é do canal configurado.
 *
 * Cada link é tratado uma vez, sozinho; o botão repete a ação se o usuário fechou a tela.
 */
import { Plus } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { ModrinthLinkTarget, PackId, SearchResult } from '../../../lib/ipc/bindings';
import { CommandError } from '../../../lib/ipc/result';
import { useModrinthLink } from './api';

export interface ModrinthLinkPanelProps {
  packId: PackId;
  url: string;
  /** Link de projeto: abrir a pré-visualização. */
  onProject: (target: ModrinthLinkTarget) => void;
  /** Link de versão: abrir o diálogo de dependências com a versão exata. */
  onVersion: (target: ModrinthLinkTarget) => void;
}

/** O projeto do link no formato de um resultado da busca, para abrir a pré-visualização. */
export function resultOfModrinthLink(target: ModrinthLinkTarget): SearchResult {
  return {
    key: `modrinth:${target.projectId}`,
    sources: [{ source: 'modrinth', projectId: target.projectId, slug: '', downloads: 0 }],
    title: target.title,
    author: '',
    summary: '',
    iconUrl: null,
    downloads: 0,
    updated: '',
    inPack: false,
    compatible: true,
    manualDownload: false,
    isLibrary: false,
  };
}

export function ModrinthLinkPanel({ packId, url, onProject, onVersion }: ModrinthLinkPanelProps) {
  const { t } = useTranslation('adicionarArquivo');
  const link = useModrinthLink(packId, url);
  const handled = useRef<string | null>(null);
  const target = link.data;

  const open = (found: ModrinthLinkTarget) => {
    if (found.versionId === null) onProject(found);
    else onVersion(found);
  };
  const openRef = useRef(open);
  useEffect(() => {
    openRef.current = open;
  });
  useEffect(() => {
    if (target && handled.current !== url) {
      handled.current = url;
      openRef.current(target);
    }
  }, [target, url]);

  if (link.isPending) {
    return (
      <div className="stack-2 preview__section" aria-busy="true">
        <LoadingState inline label={t('link.lendoModrinth')} />
      </div>
    );
  }
  if (link.isError) {
    const unsupportedType =
      link.error instanceof CommandError && link.error.appError.params.reason === 'unsupportedType';
    if (unsupportedType) {
      return <Alert kind="warn" title={t('link.tipoNaoAceito')} />;
    }
    return (
      <ErrorPanel
        title={t('link.erroModrinth')}
        error={link.error}
        onRetry={() => {
          void link.refetch();
        }}
      />
    );
  }
  const isVersion = link.data.versionId !== null;
  return (
    <Alert
      kind="ok"
      role="status"
      title={t(isVersion ? 'link.versaoLida' : 'link.projetoLido', { name: link.data.title })}
      actions={
        <Button
          size="sm"
          {...(isVersion ? { icon: Plus } : {})}
          onClick={() => {
            open(link.data);
          }}
        >
          {isVersion
            ? t('link.adicionarVersao', { name: link.data.title })
            : t('link.verProjeto', { name: link.data.title })}
        </Button>
      }
    />
  );
}
