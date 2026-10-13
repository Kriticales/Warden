/**
 * Travamentos na seção Problemas (SPEC T14; ROADMAP D-06; protótipo `problemas`, tabela
 * `crashesTable`): os testes que travaram agrupados pela causa (assinatura), com quantas vezes,
 * a última vez e as versões do pack, a situação, "Ver o que causou" (o resultado do teste mais
 * novo do grupo) e cada vez que travou. Substitui o "Último travamento" da D-03.
 *
 * A retenção dos travamentos guardados avisa perto do limite e, passando dele, oferece apagar
 * os mais antigos (com confirmação).
 */
import { Link } from '@tanstack/react-router';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { CrashGroupView, CrashRetention, PackId } from '../../../lib/ipc/bindings';
import { formatMemory, formatWhen } from '../../test/text';
import type { ProblemsBlockProps } from '../problems-blocks';
import { useCrash, useCrashes, usePruneCrashes } from './api';
import { crashTitle, situationKind, situationText, versionLabel, versionsText } from './text';
import './crashes.css';

const MIB = 1024 * 1024;

export function CrashesBlock({ packId }: ProblemsBlockProps) {
  const { t } = useTranslation('travamentos');
  const crashes = useCrashes(packId);
  const limite = formatMemory((crashes.data?.retention.budgetBytes ?? 500 * MIB) / MIB);

  return (
    <section className="stack-2" aria-labelledby="diag-crashes-title">
      <h2 className="group-title" id="diag-crashes-title">
        {t('titulo')} <span className="t-3">{t('subtitulo')}</span>
      </h2>
      {crashes.isPending ? (
        <LoadingState label={t('carregando')} />
      ) : crashes.isError ? (
        <ErrorPanel
          title={t('erro')}
          error={crashes.error}
          compact
          onRetry={() => {
            void crashes.refetch();
          }}
        />
      ) : (
        <>
          <RetentionNotice packId={packId} retention={crashes.data.retention} limite={limite} />
          {crashes.data.groups.length === 0 ? (
            <p className="t-sm t-3">{t('vazio')}</p>
          ) : (
            <div className="tablewrap">
              <table className="table diag-crashes">
                <thead>
                  <tr>
                    <th>{t('colunas.causa')}</th>
                    <th>{t('colunas.vezes')}</th>
                    <th>{t('colunas.ultima')}</th>
                    <th>{t('colunas.situacao')}</th>
                    <th className="shrink">
                      <span className="sr-only">{t('colunas.acoes')}</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {crashes.data.groups.map((group) => (
                    <CrashRow key={group.signature.hash} packId={packId} group={group} />
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <p className="t-xs t-3">{t('rodape', { limite })}</p>
        </>
      )}
    </section>
  );
}

function CrashRow({ packId, group }: { packId: PackId; group: CrashGroupView }) {
  const { t } = useTranslation('travamentos');
  const [open, setOpen] = useState(false);
  const listId = useId();
  const quando = formatWhen(group.lastAtMs);
  return (
    <>
      <tr>
        <td className="t-strong">{crashTitle(group)}</td>
        <td className="t-2">{t('vezes', { count: group.count })}</td>
        <td className="t-2">
          {quando}
          <div className="t-xs t-3">{versionsText(group.versions)}</div>
        </td>
        <td>
          <span className={`status status--${situationKind(group.situation)} diag-crashes__status`}>
            {situationText(group.situation)}
          </span>
        </td>
        <td className="shrink">
          <div className="row row--wrap diag-crashes__actions">
            <Button size="sm" asChild>
              <Link
                to="/packs/$packId/teste"
                params={{ packId }}
                search={{ sessao: group.latestSessionId }}
                aria-label={t('verCausaRotulo', { quando })}
              >
                {t('verCausa')}
              </Link>
            </Button>
            {group.count > 1 ? (
              <Button
                size="sm"
                variant="ghost"
                icon={open ? ChevronDown : ChevronRight}
                aria-expanded={open}
                aria-controls={listId}
                onClick={() => {
                  setOpen((value) => !value);
                }}
              >
                {open ? t('ocultarVezes') : t('verVezes', { count: group.count })}
              </Button>
            ) : null}
          </div>
        </td>
      </tr>
      {open ? (
        <tr>
          <td colSpan={5} id={listId}>
            <Occurrences packId={packId} signature={group.signature.hash} />
          </td>
        </tr>
      ) : null}
    </>
  );
}

/** Cada vez que o jogo travou com essa causa, com o link para o resultado do teste. */
function Occurrences({ packId, signature }: { packId: PackId; signature: string }) {
  const { t } = useTranslation('travamentos');
  const detail = useCrash(packId, signature, true);
  if (detail.isPending) {
    return <LoadingState label={t('vezesCarregando')} />;
  }
  if (detail.isError) {
    return (
      <ErrorPanel
        title={t('erro')}
        error={detail.error}
        compact
        onRetry={() => {
          void detail.refetch();
        }}
      />
    );
  }
  return (
    <div>
      <p className="t-xs t-3">{t('vezesTitulo')}</p>
      <ul className="diag-crashes__occurrences">
        {(detail.data?.occurrences ?? []).map((occurrence) => {
          const quando = formatWhen(occurrence.startedAtMs);
          return (
            <li key={occurrence.sessionId}>
              <Link
                to="/packs/$packId/teste"
                params={{ packId }}
                search={{ sessao: occurrence.sessionId }}
                aria-label={t('abrirVez', { quando })}
              >
                {t('vez', { quando, versao: versionLabel(occurrence.version) })}
              </Link>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Aviso perto do limite e, passando dele, "Apagar os mais antigos" com confirmação. */
function RetentionNotice({
  packId,
  retention,
  limite,
}: {
  packId: PackId;
  retention: CrashRetention;
  limite: string;
}) {
  const { t } = useTranslation('travamentos');
  const prune = usePruneCrashes(packId);
  const count = retention.prunable.length;
  if (count > 0) {
    return (
      <Alert
        kind="warn"
        title={t('retencao.passouTitulo', { limite })}
        actions={
          <ConfirmDialog
            trigger={<Button size="sm">{t('retencao.apagar')}</Button>}
            title={t('retencao.confirmarTitulo', { count })}
            description={t('retencao.confirmarTexto')}
            confirmLabel={t('retencao.confirmar', { count })}
            confirmingLabel={t('retencao.apagando')}
            onConfirm={async () => {
              await prune.mutateAsync(retention.prunable);
            }}
          />
        }
      >
        {t('retencao.passou', { count })}
      </Alert>
    );
  }
  if (retention.overLimit) {
    return (
      <Alert kind="warn" title={t('retencao.passouTitulo', { limite })}>
        {t('retencao.passouSemApagar')}
      </Alert>
    );
  }
  if (retention.nearLimit) {
    return (
      <Alert kind="info" title={t('retencao.pertoTitulo', { limite })}>
        {t('retencao.perto')}
      </Alert>
    );
  }
  return null;
}
