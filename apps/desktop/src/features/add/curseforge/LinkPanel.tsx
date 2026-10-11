/**
 * Link da CurseForge colado no campo único (SPEC T08 "Link"; CA-T08-03): a tela lê o link pelo
 * backend e segue pelo caminho normal.
 *
 * - **Projeto:** abre a pré-visualização com o seletor de versão, igual à busca.
 * - **Arquivo:** o packwiz lê o link numa cópia do pack (nada é gravado) e a tela abre o
 *   diálogo de dependências com aquele arquivo exato.
 *
 * Cada link é tratado uma vez, sozinho; os botões repetem a ação se o usuário fechou a tela.
 */
import { Plus } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from '@tanstack/react-router';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button, buttonVariants } from '../../../components/ui/button';
import { CommandError } from '../../../lib/ipc/result';
import type { LinkTarget, PackId } from '../../../lib/ipc/bindings';
import { useCurseforgeLink } from './api';

export interface CurseforgeLinkPanelProps {
  packId: PackId;
  /** O link (já conferido por `parseCurseforgeLink`). */
  url: string;
  /** Link de projeto: abrir a pré-visualização. */
  onProject: (target: LinkTarget) => void;
  /** Link de arquivo: abrir o diálogo de dependências com o arquivo exato. */
  onFile: (target: LinkTarget) => void;
}

export function CurseforgeLinkPanel({ packId, url, onProject, onFile }: CurseforgeLinkPanelProps) {
  const { t } = useTranslation('adicionarCurseforge');
  const link = useCurseforgeLink(packId, url);
  const handled = useRef<string | null>(null);
  const target = link.data;

  const open = (found: LinkTarget) => {
    if (found.fileId === null) onProject(found);
    else onFile(found);
  };
  // Cada link abre sozinho uma vez; o botão repete.
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
        <LoadingState inline label={t('link.lendo')} />
        <p className="t-xs t-3">{t('link.lendoArquivo')}</p>
      </div>
    );
  }
  if (link.isError) {
    const keyMissing =
      link.error instanceof CommandError &&
      link.error.appError.code.code === 'CURSEFORGE_KEY_MISSING';
    return (
      <ErrorPanel
        title={t('link.erro')}
        error={link.error}
        onRetry={() => {
          void link.refetch();
        }}
        actions={
          keyMissing ? (
            <Link to="/configuracoes" className={buttonVariants({ size: 'sm' })}>
              {t('link.abrirConfiguracoes')}
            </Link>
          ) : undefined
        }
      />
    );
  }
  const isFile = link.data.fileId !== null;
  return (
    <Alert
      kind="ok"
      title={t(isFile ? 'link.arquivoLido' : 'link.projetoLido', { name: link.data.title })}
      role="status"
      actions={
        <Button
          size="sm"
          icon={isFile ? Plus : undefined}
          onClick={() => {
            open(link.data);
          }}
        >
          {isFile
            ? t('link.adicionarArquivo', { name: link.data.title })
            : t('link.verProjeto', { name: link.data.title })}
        </Button>
      }
    />
  );
}
