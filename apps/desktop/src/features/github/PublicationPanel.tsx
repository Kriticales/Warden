/**
 * A área "Publicação para os jogadores" do Histórico (SPEC T17 e T18; protótipo `historico`):
 * repositório e visibilidade, a última versão publicada, o link do pack com "Copiar link" e
 * "Como os jogadores instalam". Antes da primeira publicação, uma frase explica como os
 * jogadores recebem o pack; com repositório privado, "Só backup" no lugar do passo a passo.
 * Versões enviadas sem a Release do GitHub aparecem aqui com "Tentar de novo".
 */
import { Copy, Globe, Lock } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button } from '../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../components/ui/dialog';
import { Icon } from '../../components/ui/icon';
import { showToast } from '../../components/ui/toast';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { formatDate } from '../../lib/format';
import type { PublicationPanelProps } from '../versioning/publication';
import { useGithubStatus, useRetryRelease } from './api';
import { copyText } from './copy';
import { PlayerSteps } from './PlayerSteps';
import './github.css';

export function PublicationPanel({ packId }: PublicationPanelProps) {
  const { t } = useTranslation('publicar');
  const status = useGithubStatus(packId);
  const retry = useRetryRelease(packId);
  const [howTo, setHowTo] = useState(false);

  const title = (
    <h2 className="panel__title" id="historico-publicacao" tabIndex={-1}>
      {t('area.titulo')}
    </h2>
  );
  if (status.isPending) {
    return (
      <section className="panel" aria-labelledby="historico-publicacao">
        {title}
        <LoadingState inline label={t('area.carregando')} />
      </section>
    );
  }
  if (status.isError) {
    return (
      <section className="panel" aria-labelledby="historico-publicacao">
        {title}
        <ErrorPanel
          compact
          title={t('area.erro')}
          error={status.error}
          onRetry={() => {
            void status.refetch();
          }}
        />
      </section>
    );
  }
  const { link, published } = status.data;
  if (link === null) {
    return (
      <section className="panel" aria-labelledby="historico-publicacao">
        {title}
        <p className="t-sm t-2 mt-2">{t('area.antes')}</p>
      </section>
    );
  }
  const latest = published[0] ?? null;
  const pending = published.filter((version) => version.releasePending);
  const game = { minecraft: status.data.minecraft, loader: status.data.loader };

  return (
    <section className="panel" aria-labelledby="historico-publicacao">
      <div className="panel__head">
        {title}
        {link.private ? null : (
          <Button
            variant="link"
            onClick={() => {
              setHowTo(true);
            }}
          >
            {t('area.comoInstalam')}
          </Button>
        )}
      </div>
      <dl className="kv mt-2">
        <dt>{t('area.repositorio')}</dt>
        <dd>
          {link.repo.owner}/{link.repo.name}{' '}
          <span className="tag tag--plain">
            <Icon icon={link.private ? Lock : Globe} />
            {link.private ? t('area.privado') : t('area.publico')}
          </span>
        </dd>
        <dt>{t('area.versaoPublicada')}</dt>
        <dd>
          {latest
            ? t('area.publicadaEm', {
                version: latest.version,
                date: formatDate(Date.parse(latest.date)),
              })
            : t('area.nenhumaPublicada')}
        </dd>
      </dl>
      {link.private ? (
        <p className="t-sm t-2 mt-2">{t('area.soBackup')}</p>
      ) : (
        <div className="field mt-3">
          <label className="field__label" htmlFor="historico-link-do-pack">
            {t('area.link')}
          </label>
          <div className="linkbox">
            <input
              id="historico-link-do-pack"
              className="input"
              readOnly
              value={link.packLink}
              aria-describedby="historico-link-dica"
            />
            <Button
              size="sm"
              icon={Copy}
              onClick={() => {
                void copyText(t, link.packLink, 'copiar.copiado');
              }}
            >
              {t('copiar.link')}
            </Button>
          </div>
          <p className="field__hint" id="historico-link-dica">
            {t('area.linkDica')}
          </p>
        </div>
      )}
      {pending.length > 0 ? (
        <Alert
          kind="warn"
          className="mt-3"
          actions={pending.map((version) => (
            <Button
              key={version.version}
              size="sm"
              loading={retry.isPending && retry.variables === version.version}
              onClick={() => {
                retry.mutate(version.version, {
                  onSuccess: (result) => {
                    showToast({
                      kind: 'ok',
                      title: t('area.releaseCriada', { version: result.version }),
                    });
                  },
                  onError: (error) => {
                    showToast({
                      kind: 'danger',
                      title: t('area.releaseFalhou'),
                      text: appErrorMessage(toAppError(error)),
                    });
                  },
                });
              }}
            >
              {pending.length > 1
                ? t('area.tentarDeNovoVersao', { version: version.version })
                : t('area.tentarDeNovo')}
            </Button>
          ))}
        >
          {t('area.releasePendente', {
            count: pending.length,
            versions: pending.map((version) => version.version).join(', '),
          })}
        </Alert>
      ) : null}
      <Dialog open={howTo} onOpenChange={setHowTo}>
        {howTo ? (
          <DialogContent
            title={t('area.comoInstalam')}
            size="lg"
            footer={
              <DialogClose asChild>
                <Button variant="primary">{t('passos.fechar')}</Button>
              </DialogClose>
            }
          >
            <PlayerSteps packLink={link.packLink} game={game} />
          </DialogContent>
        ) : null}
      </Dialog>
    </section>
  );
}
