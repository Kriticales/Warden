/**
 * Link direto `https://` para um `.jar`/`.zip` (SPEC T08 "Link"; P1-11): o Warden baixa uma
 * vez (cancelável), calcula o hash, lê os metadados e segue como um arquivo do computador
 * ([`LocalFilesPanel`]): pede o tipo, oferece a referência quando o arquivo é achado pelo hash
 * e grava pelo link (`[download]` com o link e o `sha256`).
 */
import { RefreshCw, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { AddResult, HashMatch, PackId } from '../../../lib/ipc/bindings';
import type { PackTarget } from '../common/text';
import { LocalFilesPanel } from '../local/LocalFilesPanel';
import { useDirectDownload } from './api';
import { directFileName, linkHost } from './link';

export interface DirectLinkPanelProps {
  packId: PackId;
  url: string;
  target: PackTarget;
  onReference: (match: HashMatch) => void;
  onAdded: (result: AddResult) => void;
  onBack: () => void;
}

export function DirectLinkPanel({
  packId,
  url,
  target,
  onReference,
  onAdded,
  onBack,
}: DirectLinkPanelProps) {
  const { t } = useTranslation('adicionarArquivo');
  const { state, cancel, retry } = useDirectDownload(packId, url);
  const host = linkHost(url);
  let file = '';
  try {
    file = directFileName(new URL(url.trim())) ?? '';
  } catch {
    file = '';
  }

  switch (state.status) {
    case 'downloading':
      return (
        <div className="stack-2 preview__section" aria-busy="true">
          <LoadingState inline label={t('link.baixando', { file })} />
          <p className="t-xs t-3">{t('link.baixandoTexto')}</p>
          <div>
            <Button size="sm" icon={X} onClick={cancel}>
              {t('link.cancelar')}
            </Button>
          </div>
        </div>
      );
    case 'cancelled':
      return (
        <Alert
          kind="neutral"
          title={t('link.cancelado')}
          actions={
            <Button size="sm" icon={RefreshCw} onClick={retry}>
              {t('link.tentarDeNovo')}
            </Button>
          }
        />
      );
    case 'error':
      return <ErrorPanel title={t('link.erroDownload')} error={state.error} onRetry={retry} />;
    case 'done':
      return (
        <LocalFilesPanel
          packId={packId}
          files={[state.file]}
          link={url.trim()}
          linkHost={host}
          target={target}
          onReference={onReference}
          onAdded={onAdded}
          onBack={onBack}
        />
      );
  }
}
