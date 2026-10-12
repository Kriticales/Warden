/**
 * Uma versão salva na linha do tempo (SPEC T17): número, estado (só salva, versão final ou
 * publicada), data, resumo do changelog e as ações: marcar e desmarcar a versão final, publicar
 * (V-03), voltar para esta versão e abrir o changelog com "Ver diferenças para o estado atual".
 */
import { ChevronDown, ChevronUp, FileDiff, Flag, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { PackId, SavedVersion } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useRestoreVersion, useSetFinal } from '../api';
import { summarizeChangelog, versionDomId, versionState } from '../model';
import { publicationSlots } from '../publication';
import { DiffDialog } from './DiffDialog';
import { TimelineItem } from './TimelineItem';

/** O resumo de uma linha: as notas do usuário ou quantos itens mudaram. */
function useChangelogSummary() {
  const { t } = useTranslation('versoes');
  return (message: string): string => {
    const summary = summarizeChangelog(message);
    if (summary.notes !== null) {
      return summary.notes;
    }
    const parts: string[] = [];
    if (summary.platform) {
      parts.push(t('versao.resumoPlataforma'));
    }
    if (summary.added > 0) {
      parts.push(t('versao.resumoAdicionados', { count: summary.added }));
    }
    if (summary.removed > 0) {
      parts.push(t('versao.resumoRemovidos', { count: summary.removed }));
    }
    if (summary.updated > 0) {
      parts.push(t('versao.resumoAtualizados', { count: summary.updated }));
    }
    if (summary.others > 0) {
      parts.push(t('versao.resumoOutros', { count: summary.others }));
    }
    if (summary.configs > 0) {
      parts.push(t('versao.resumoConfigs', { count: summary.configs }));
    }
    return parts.length > 0 ? parts.join(' · ') : t('versao.semResumo');
  };
}

export interface VersionItemProps {
  packId: PackId;
  version: SavedVersion;
  /** Começa com os detalhes abertos. */
  defaultOpen?: boolean;
  /** O pack aceita escritas (não está somente leitura). */
  canWrite: boolean;
  /** Voltar para esta versão tem efeito (o pack não está igual a ela). */
  canRestore: boolean;
  /** Quantas alterações não salvas o pack tem agora (para o aviso de voltar). */
  unsavedCount: number;
}

export function VersionItem({
  packId,
  version,
  defaultOpen = false,
  canWrite,
  canRestore,
  unsavedCount,
}: VersionItemProps) {
  const { t } = useTranslation('versoes');
  const summarize = useChangelogSummary();
  const [open, setOpen] = useState(defaultOpen);
  const [diffing, setDiffing] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const setFinal = useSetFinal(packId);
  const restore = useRestoreVersion(packId);
  const Publish = publicationSlots.publishButton;
  const state = versionState(version);
  const id = versionDomId(version.version);

  const changeFinal = (isFinal: boolean) => {
    setFinal.mutate(
      { version: version.version, isFinal },
      {
        onSuccess: () => {
          showToast({
            kind: 'ok',
            title: isFinal
              ? t('versao.marcada', { version: version.version })
              : t('versao.desmarcada', { version: version.version }),
          });
        },
        onError: (error) => {
          showToast({
            kind: 'danger',
            title: isFinal ? t('versao.marcarFinal') : t('versao.desmarcar'),
            text: appErrorMessage(toAppError(error)),
          });
        },
      },
    );
  };

  const actions = (
    <>
      {state === 'saved' ? (
        <Button
          size="sm"
          icon={Flag}
          disabled={!canWrite}
          loading={setFinal.isPending}
          onClick={() => {
            changeFinal(true);
          }}
        >
          {t('versao.marcarFinal')}
        </Button>
      ) : null}
      {state === 'final' ? (
        <>
          <Publish packId={packId} version={version.version} />
          <Button
            size="sm"
            variant="ghost"
            disabled={!canWrite}
            loading={setFinal.isPending}
            onClick={() => {
              changeFinal(false);
            }}
          >
            {t('versao.desmarcar')}
          </Button>
        </>
      ) : null}
      {canRestore ? (
        <Button
          size="sm"
          icon={RotateCcw}
          disabled={!canWrite}
          aria-label={t('versao.voltarRotulo', { version: version.version })}
          onClick={() => {
            setRestoring(true);
          }}
        >
          {t('versao.voltar')}
        </Button>
      ) : null}
      <Button
        size="sm"
        variant="ghost"
        icon={open ? ChevronUp : ChevronDown}
        aria-expanded={open}
        aria-controls={`${id}-detalhe`}
        onClick={() => {
          setOpen((current) => !current);
        }}
      >
        {open ? t('versao.fecharDetalhes') : t('versao.verMudancas')}
      </Button>
    </>
  );

  const detail = open ? (
    <div id={`${id}-detalhe`} className="stack-3">
      {version.message.trim() === '' ? (
        <p className="t-sm t-3">{t('versao.semChangelog')}</p>
      ) : (
        <pre
          className="code code--scroll"
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
          tabIndex={0}
          aria-label={t('versao.changelog', { version: version.version })}
        >
          {version.message.trim()}
        </pre>
      )}
      <div className="btn-row">
        <Button
          size="sm"
          icon={FileDiff}
          onClick={() => {
            setDiffing(true);
          }}
        >
          {t('versao.verDiferencas')}
        </Button>
      </div>
    </div>
  ) : null;

  return (
    <>
      <TimelineItem
        id={id}
        kind={state}
        version={version.version}
        date={version.date}
        summary={summarize(version.message)}
        actions={actions}
        detail={detail}
        open={open}
      />
      {diffing ? (
        <DiffDialog
          packId={packId}
          version={version.version}
          onClose={() => {
            setDiffing(false);
          }}
        />
      ) : null}
      <ConfirmDialog
        open={restoring}
        onOpenChange={setRestoring}
        title={t('voltar.titulo', { version: version.version })}
        description={
          <>
            {t('voltar.texto', { version: version.version })}
            <br />
            {unsavedCount > 0
              ? t('voltar.pontoDeSeguranca', { count: unsavedCount })
              : t('voltar.pontoDeSegurancaLimpo')}{' '}
            {t('voltar.depois')}
          </>
        }
        confirmLabel={t('voltar.confirmar', { version: version.version })}
        confirmingLabel={t('voltar.voltando')}
        destructive={false}
        onConfirm={async () => {
          await restore.mutateAsync(version.version);
          showToast({
            kind: 'ok',
            title: t('voltar.feito', { version: version.version }),
            text: t('voltar.feitoTexto'),
          });
        }}
      />
    </>
  );
}
