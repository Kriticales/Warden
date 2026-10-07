/**
 * Versões da pré-visualização (SPEC T08, P1; protótipo `pv-ver-h`): as versões que servem para
 * o pack, cada uma numa linha que abre e mostra as notas. As notas só são pedidas à fonte
 * quando a linha é aberta (CA-T08-13: na CurseForge, uma chamada por linha aberta).
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { SafeHtml } from '../../../components/common/SafeHtml';
import { SafeMarkdown } from '../../../components/common/SafeMarkdown';
import { Button } from '../../../components/ui/button';
import type { PackId, ProjectVersions, SourceId, VersionOption } from '../../../lib/ipc/bindings';
import { type PackTarget, useAgoText, useTargetText } from '../common/text';
import { useVersionNotes } from './api';

export interface VersionsProps {
  packId: PackId;
  source: SourceId;
  projectId: string;
  versions: ProjectVersions;
  target: PackTarget;
  /** A versão que vai entrar no pack. */
  chosenId: string | null;
  onChoose: (versionId: string) => void;
}

export function Versions({
  packId,
  source,
  projectId,
  versions,
  target,
  chosenId,
  onChoose,
}: VersionsProps) {
  const { t } = useTranslation('descoberta');
  const alvo = useTargetText(target);
  if (versions.versions.length === 0) return <p className="t-sm t-3">{t('versoes.nenhuma')}</p>;
  return (
    <div>
      <p className="t-xs t-3">{t('versoes.titulo', { alvo })}</p>
      <ul className="stack-2 versions" aria-label={t('versoes.lista')}>
        {versions.versions.map((version) => (
          <VersionRow
            key={version.id}
            packId={packId}
            source={source}
            projectId={projectId}
            version={version}
            newest={version.id === versions.defaultId}
            chosen={version.id === chosenId}
            onChoose={onChoose}
          />
        ))}
      </ul>
    </div>
  );
}

interface VersionRowProps {
  packId: PackId;
  source: SourceId;
  projectId: string;
  version: VersionOption;
  newest: boolean;
  chosen: boolean;
  onChoose: (versionId: string) => void;
}

function VersionRow({
  packId,
  source,
  projectId,
  version,
  newest,
  chosen,
  onChoose,
}: VersionRowProps) {
  const { t } = useTranslation('descoberta');
  const agoText = useAgoText();
  // Uma vez aberta, a linha continua pedindo (e guardando) as notas.
  const [opened, setOpened] = useState(false);
  const published = agoText(version.published);
  return (
    <li>
      <details
        className="disclosure"
        onToggle={(event) => {
          if (event.currentTarget.open) setOpened(true);
        }}
      >
        <summary>
          <span className="t-mono">{version.number}</span>
          {' · '}
          {t(`versoes.canal.${version.channel}`)}
          {published ? ` · ${published}` : ''}
          {newest ? ` · ${t('versoes.maisNova')}` : ''}
          {chosen ? ` · ${t('versoes.selecionada')}` : ''}
        </summary>
        {opened ? (
          <VersionNotes
            packId={packId}
            source={source}
            projectId={projectId}
            versionId={version.id}
          />
        ) : null}
        {chosen ? null : (
          <Button
            size="sm"
            onClick={() => {
              onChoose(version.id);
            }}
          >
            {t('versoes.usar')}
          </Button>
        )}
      </details>
    </li>
  );
}

function VersionNotes({
  packId,
  source,
  projectId,
  versionId,
}: {
  packId: PackId;
  source: SourceId;
  projectId: string;
  versionId: string;
}) {
  const { t } = useTranslation('descoberta');
  const notes = useVersionNotes(packId, source, projectId, versionId, true);
  if (notes.isPending) return <LoadingState inline label={t('versoes.notasCarregando')} />;
  if (notes.isError) {
    return (
      <ErrorPanel
        compact
        title={t('versoes.notasErro')}
        error={notes.error}
        onRetry={() => {
          void notes.refetch();
        }}
      />
    );
  }
  if (notes.data === null) return <p className="t-sm t-3">{t('versoes.semNotas')}</p>;
  return notes.data.format === 'html' ? (
    <SafeHtml html={notes.data.body} />
  ) : (
    <SafeMarkdown>{notes.data.body}</SafeMarkdown>
  );
}
