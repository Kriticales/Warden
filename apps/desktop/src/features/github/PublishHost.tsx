/**
 * O fluxo "Publicar a versão X para os jogadores" (SPEC T18; protótipo `publicar`,
 * `publicar-repo` e `publicado`), montado no cabeçalho do pack para continuar aberto quando o
 * botão da versão some:
 *
 * 1. sem token do GitHub: o caminho até Configurações;
 * 2. primeira publicação: o repositório (público recomendado, privado "só backup", ou um que já
 *    existe);
 * 3. o diálogo com o que bloqueia, os avisos (com "Trocar pelo Modrinth" e "Testar agora"), as
 *    notas desde a última publicada, o recado para os jogadores e o que vai e o que não vai;
 *    com o GitHub divergente, "Substituir o GitHub pelo que está aqui" com dupla confirmação;
 * 4. o resultado: link do pack, passo a passo dos jogadores, o espaço da instância pronta para
 *    o Prism (E-04), "Copiar texto das notas da versão" e "Abrir a Release no GitHub".
 */
import { Link } from '@tanstack/react-router';
import { Copy, ExternalLink, Globe, Play, Settings } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import { Button, buttonVariants } from '../../components/ui/button';
import { Dialog, DialogContent } from '../../components/ui/dialog';
import { Icon } from '../../components/ui/icon';
import { showToast } from '../../components/ui/toast';
import { Tooltip } from '../../components/ui/tooltip';
import type {
  GithubStatus,
  PackId,
  PublishFinding,
  PublishMod,
  PublishPlan,
  Published,
} from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { commandError } from '../../lib/ipc/query';
import { openExternal } from '../../lib/external';
import type { HeaderSlotProps } from '../pack-editor/header/slots';
import { useHygieneFix } from '../packs/api';
import { Choice, TextField } from '../settings/components/fields';
import {
  useGithubSetup,
  useGithubStatus,
  usePublishPlan,
  usePublishRun,
  useRetryRelease,
  useSwapToModrinth,
} from './api';
import { copyText } from './copy';
import { isValidRepoName, isValidRepoRef, splitFindings } from './model';
import { PlayerSteps } from './PlayerSteps';
import { githubSlots } from './slots';
import { usePublishStore } from './store';
import './github.css';

/** Grupo dos rádios de visibilidade do repositório. */
const VISIBILITY_GROUP = 'publicar-visibilidade';

export function PublishHost({ packId, pack }: HeaderSlotProps) {
  const request = usePublishStore((store) => store.request);
  const close = usePublishStore((store) => store.close);
  const [result, setResult] = useState<Published | null>(null);
  // As mutations ficam aqui, que não desmonta ao publicar.
  const run = usePublishRun(packId);
  const retry = useRetryRelease(packId);
  const active = request !== null && request.packId === packId;
  const opener = request?.opener ?? null;
  const returnFocus = () =>
    opener?.isConnected ? opener : document.getElementById('historico-publicacao');

  const finish = (published: Published) => {
    setResult(published);
    close();
  };

  return (
    <>
      <Dialog
        open={active}
        onOpenChange={(open) => {
          if (!open && !run.isPending) {
            run.reset();
            retry.reset();
            close();
          }
        }}
      >
        {active ? (
          <PublishSteps
            packId={packId}
            packName={pack.name}
            version={request.version}
            run={run}
            retry={retry}
            returnFocus={returnFocus}
            onPublished={finish}
            onClose={close}
          />
        ) : null}
      </Dialog>
      <Dialog
        open={result !== null}
        onOpenChange={(open) => {
          if (!open) {
            setResult(null);
          }
        }}
      >
        {result ? (
          <ResultContent
            packId={packId}
            published={result}
            returnFocus={returnFocus}
            onClose={() => {
              setResult(null);
            }}
          />
        ) : null}
      </Dialog>
    </>
  );
}

type RunMutation = ReturnType<typeof usePublishRun>;
type RetryMutation = ReturnType<typeof useRetryRelease>;

interface StepsProps {
  packId: PackId;
  packName: string;
  version: string;
  run: RunMutation;
  retry: RetryMutation;
  returnFocus: () => HTMLElement | null;
  onPublished: (published: Published) => void;
  onClose: () => void;
}

function PublishSteps(props: StepsProps) {
  const { t } = useTranslation('publicar');
  const status = useGithubStatus(props.packId);
  if (status.isPending) {
    return (
      <DialogContent
        title={t('plano.titulo', { version: props.version })}
        returnFocusTo={props.returnFocus}
      >
        <LoadingState inline label={t('area.carregando')} />
      </DialogContent>
    );
  }
  if (status.isError) {
    return (
      <DialogContent
        title={t('plano.titulo', { version: props.version })}
        returnFocusTo={props.returnFocus}
        footer={
          <Button variant="ghost" onClick={props.onClose}>
            {t('plano.cancelar')}
          </Button>
        }
      >
        <ErrorPanel
          title={t('area.erro')}
          error={status.error}
          onRetry={() => {
            void status.refetch();
          }}
        />
      </DialogContent>
    );
  }
  if (!status.data.tokenConfigured) {
    return <TokenMissing {...props} />;
  }
  if (status.data.link === null) {
    return <RepoSetup {...props} status={status.data} />;
  }
  return <PlanContent {...props} status={status.data} />;
}

function TokenMissing({ returnFocus, onClose }: StepsProps) {
  const { t } = useTranslation('publicar');
  return (
    <DialogContent
      title={t('token.titulo')}
      size="sm"
      returnFocusTo={returnFocus}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('plano.cancelar')}
          </Button>
          <Link
            to="/configuracoes"
            className={buttonVariants({ variant: 'primary' })}
            onClick={onClose}
          >
            <Icon icon={Settings} />
            <span>{t('token.abrir')}</span>
          </Link>
        </>
      }
    >
      <p>{t('token.texto')}</p>
    </DialogContent>
  );
}

function RepoSetup({
  packId,
  returnFocus,
  onClose,
  status,
}: StepsProps & { status: GithubStatus }) {
  const { t } = useTranslation('publicar');
  const setup = useGithubSetup(packId);
  const [mode, setMode] = useState<'create' | 'link'>('create');
  const [name, setName] = useState(status.suggestedName);
  const [isPrivate, setPrivate] = useState(false);
  const [repo, setRepo] = useState('');
  const nameOk = isValidRepoName(name.trim());
  const repoOk = isValidRepoRef(repo);
  const ready = mode === 'create' ? nameOk : repoOk;

  const submit = () => {
    if (!ready || setup.isPending) {
      return;
    }
    setup.mutate(
      mode === 'create'
        ? { kind: 'create', name: name.trim(), private: isPrivate }
        : { kind: 'link', repo: repo.trim() },
    );
  };

  return (
    <DialogContent
      title={t('repo.titulo')}
      description={t('repo.descricao')}
      returnFocusTo={returnFocus}
      footer={
        <>
          <Button variant="ghost" disabled={setup.isPending} onClick={onClose}>
            {t('repo.cancelar')}
          </Button>
          <Button
            variant="primary"
            icon={Globe}
            loading={setup.isPending}
            disabled={!ready}
            onClick={submit}
          >
            {mode === 'create' ? t('repo.criar') : t('repo.vincular')}
          </Button>
        </>
      }
    >
      <form
        className="stack-3"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        {mode === 'create' ? (
          <>
            <TextField
              label={t('repo.nome')}
              mono
              value={name}
              hint={t('repo.nomeDica')}
              error={name.trim() !== '' && !nameOk ? t('repo.nomeInvalido') : null}
              onChange={(event) => {
                setName(event.target.value);
              }}
            />
            <fieldset className="m-0 border-0 p-0">
              <legend className="field__label">{t('repo.quemVe')}</legend>
              <div className="choice-list">
                <Choice
                  name={VISIBILITY_GROUP}
                  title={t('repo.publico')}
                  badge={<span className="tag tag--ok">{t('repo.recomendado')}</span>}
                  desc={t('repo.publicoDesc')}
                  checked={!isPrivate}
                  onSelect={() => {
                    setPrivate(false);
                  }}
                />
                <Choice
                  name={VISIBILITY_GROUP}
                  title={t('repo.privado')}
                  desc={t('repo.privadoDesc')}
                  checked={isPrivate}
                  onSelect={() => {
                    setPrivate(true);
                  }}
                />
              </div>
            </fieldset>
          </>
        ) : (
          <TextField
            label={t('repo.existente')}
            mono
            value={repo}
            hint={t('repo.existenteDica')}
            onChange={(event) => {
              setRepo(event.target.value);
            }}
          />
        )}
        <div>
          <Button
            variant="link"
            onClick={() => {
              setup.reset();
              setMode(mode === 'create' ? 'link' : 'create');
            }}
          >
            {mode === 'create' ? t('repo.usarExistente') : t('repo.criarNovo')}
          </Button>
        </div>
        {setup.isError ? <ErrorPanel compact error={setup.error} /> : null}
        <button type="submit" hidden />
      </form>
    </DialogContent>
  );
}

function PlanContent(props: StepsProps & { status: GithubStatus }) {
  const { t } = useTranslation('publicar');
  const { packId, version, returnFocus, onClose } = props;
  const plan = usePublishPlan(packId, version, true);
  if (plan.isPending) {
    return (
      <DialogContent title={t('plano.titulo', { version })} size="lg" returnFocusTo={returnFocus}>
        <LoadingState inline label={t('plano.carregando', { version })} />
      </DialogContent>
    );
  }
  if (plan.isError) {
    return (
      <DialogContent
        title={t('plano.titulo', { version })}
        size="lg"
        returnFocusTo={returnFocus}
        footer={
          <Button variant="ghost" onClick={onClose}>
            {t('plano.cancelar')}
          </Button>
        }
      >
        <ErrorPanel
          title={t('plano.erro')}
          error={plan.error}
          onRetry={() => {
            void plan.refetch();
          }}
        />
      </DialogContent>
    );
  }
  return (
    <PlanView
      {...props}
      plan={plan.data}
      onRecheck={() => {
        void plan.refetch();
      }}
    />
  );
}

function PlanView({
  packId,
  packName,
  version,
  run,
  retry,
  returnFocus,
  onPublished,
  onClose,
  plan,
  onRecheck,
}: StepsProps & { status: GithubStatus; plan: PublishPlan; onRecheck: () => void }) {
  const { t } = useTranslation('publicar');
  const notesId = useId();
  const [playerNotes, setPlayerNotes] = useState('');
  const [replacing, setReplacing] = useState(false);
  const { blocking, warnings } = splitFindings(plan.findings);
  const diverged = plan.remote !== null && !plan.remote.inSync;
  const repo = plan.link ? `${plan.link.repo.owner}/${plan.link.repo.name}` : '';
  const runError = run.isError ? commandError(run.error) : null;
  const releasePending =
    runError?.code.domain === 'versioning' && runError.code.code === 'RELEASE_PENDING';

  const publish = (replaceRemote: PublishPlan['remote'] | null) => {
    run.mutate(
      {
        version,
        playerNotes,
        replaceRemote: replaceRemote ? { expectedMain: replaceRemote.remoteMain } : null,
      },
      {
        onSuccess: onPublished,
        onError: (error) => {
          const code = commandError(error)?.code;
          if (code?.domain === 'versioning' && code.code === 'REMOTE_DIVERGED') {
            onRecheck();
          }
        },
      },
    );
  };

  const publishLabel = warnings.length > 0 ? t('plano.publicarMesmoAssim') : t('plano.publicar');
  const publishButton = (
    <Button
      variant="primary"
      icon={Globe}
      loading={run.isPending}
      disabled={plan.blocked || diverged || releasePending}
      onClick={() => {
        publish(null);
      }}
    >
      {run.isPending ? t('plano.publicando') : publishLabel}
    </Button>
  );

  return (
    <DialogContent
      title={t('plano.titulo', { version })}
      description={t('plano.descricao', { repo })}
      size="lg"
      returnFocusTo={returnFocus}
      footer={
        <>
          <Button variant="ghost" disabled={run.isPending} onClick={onClose}>
            {t('plano.cancelar')}
          </Button>
          {plan.blocked ? (
            <Tooltip content={t('plano.bloqueado')}>
              <span>{publishButton}</span>
            </Tooltip>
          ) : (
            publishButton
          )}
        </>
      }
    >
      <div className="stack-3">
        {plan.link?.private ? (
          <Alert kind="info" compact>
            {t('repo.vinculadoPrivado', { repo })}
          </Alert>
        ) : null}
        {diverged ? (
          <Alert
            kind="danger"
            title={t('divergencia.titulo')}
            actionsBelow
            actions={
              <Button
                size="sm"
                variant="danger"
                disabled={plan.blocked || run.isPending}
                onClick={() => {
                  setReplacing(true);
                }}
              >
                {t('divergencia.substituir')}
              </Button>
            }
          >
            {t('divergencia.texto')}
          </Alert>
        ) : null}
        {blocking.length > 0 ? (
          <section aria-labelledby="publicar-bloqueios">
            <h3 className="t-caps t-3" id="publicar-bloqueios">
              {t('plano.bloqueios')}
            </h3>
            <div className="stack-2 mt-2">
              <BlockingFindings packId={packId} findings={blocking} />
            </div>
            <p className="t-xs t-3 mt-2">{t('plano.bloqueiosDica')}</p>
          </section>
        ) : null}
        <section aria-labelledby="publicar-avisos">
          <h3 className="t-caps t-3" id="publicar-avisos">
            {t('plano.avisos')}
          </h3>
          <div className="stack-2 mt-2">
            {warnings.length === 0 ? (
              <p className="t-sm t-2">{t('plano.semAvisos')}</p>
            ) : (
              warnings.map((finding, index) => (
                <WarningFinding
                  key={`${finding.kind}-${String(index)}`}
                  packId={packId}
                  version={version}
                  finding={finding}
                  onClose={onClose}
                />
              ))
            )}
          </div>
          {warnings.some((f) => f.kind === 'curseforgeBlocked' || f.kind === 'curseforgeOnly') ? (
            <p className="t-xs t-3 mt-2">{t('plano.trocaMudaPack')}</p>
          ) : null}
        </section>
        <div>
          <div className="field__label">{t('plano.notas')}</div>
          <p className="field__hint mb-2">
            {plan.lastPublished
              ? t('plano.notasDesde', { version: plan.lastPublished })
              : t('plano.notasPrimeira')}
          </p>
          <pre
            className="code code--scroll"
            // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
            tabIndex={0}
            aria-label={t('plano.notas')}
          >
            {plan.notes.trim() === '' ? t('plano.notasVazias') : plan.notes.trim()}
          </pre>
        </div>
        <div className="field">
          <label className="field__label" htmlFor={notesId}>
            {t('plano.recado')} <span className="opt">{t('plano.opcional')}</span>
          </label>
          <textarea
            id={notesId}
            className="textarea"
            rows={2}
            value={playerNotes}
            placeholder={t('plano.recadoPlaceholder')}
            onChange={(event) => {
              setPlayerNotes(event.target.value);
            }}
          />
        </div>
        <div className="grid-2 t-sm">
          <div>
            <div className="t-caps t-3">{t('plano.vai')}</div>
            <p className="t-2 mt-1">
              {t('plano.vaiTexto', { count: Math.max(plan.files - 2, 0) })}
            </p>
          </div>
          <div>
            <div className="t-caps t-3">{t('plano.naoVai')}</div>
            <p className="t-2 mt-1">{t('plano.naoVaiTexto')}</p>
          </div>
        </div>
        {run.isError ? (
          <ErrorPanel
            compact
            error={run.error}
            actions={
              releasePending ? (
                <Button
                  size="sm"
                  loading={retry.isPending}
                  onClick={() => {
                    retry.mutate(version, { onSuccess: onPublished });
                  }}
                >
                  {t('area.tentarDeNovo')}
                </Button>
              ) : undefined
            }
          />
        ) : null}
        {retry.isError ? <ErrorPanel compact error={retry.error} /> : null}
      </div>
      <ConfirmDialog
        open={replacing}
        onOpenChange={setReplacing}
        title={t('divergencia.confirmarTitulo')}
        description={t('divergencia.confirmarTexto', { version })}
        confirmText={packName}
        confirmLabel={t('divergencia.confirmar')}
        confirmingLabel={t('divergencia.substituindo')}
        onConfirm={() => {
          setReplacing(false);
          publish(plan.remote);
        }}
      />
    </DialogContent>
  );
}

function BlockingFindings({ packId, findings }: { packId: PackId; findings: PublishFinding[] }) {
  const { t } = useTranslation('publicar');
  const fix = useHygieneFix();
  const hygienePaths = findings.flatMap((finding) =>
    finding.kind === 'hygiene' ? [finding.finding.path] : [],
  );
  return (
    <>
      {findings.map((finding) => {
        if (finding.kind === 'secretFound') {
          return (
            <Alert key={`segredo-${finding.path}-${String(finding.line)}`} kind="danger" compact>
              {t('achado.segredo', { path: finding.path, line: finding.line })}
            </Alert>
          );
        }
        if (finding.kind === 'hygiene') {
          return (
            <Alert key={`higiene-${finding.finding.path}`} kind="danger" compact>
              {t('achado.higiene', { path: finding.finding.path })}
            </Alert>
          );
        }
        return null;
      })}
      {hygienePaths.length > 0 ? (
        <div>
          <Button
            size="sm"
            loading={fix.isPending}
            onClick={() => {
              fix.mutate(
                { packId, paths: hygienePaths },
                {
                  onSuccess: (deleted) => {
                    showToast({
                      kind: 'ok',
                      title: t('achado.limpo', { count: deleted.length }),
                    });
                  },
                },
              );
            }}
          >
            {t('achado.limpar')}
          </Button>
          {fix.isError ? <ErrorPanel compact error={fix.error} /> : null}
        </div>
      ) : null}
    </>
  );
}

function WarningFinding({
  packId,
  version,
  finding,
  onClose,
}: {
  packId: PackId;
  version: string;
  finding: PublishFinding;
  onClose: () => void;
}) {
  const { t } = useTranslation('publicar');
  switch (finding.kind) {
    case 'curseforgeBlocked':
      return (
        <Alert
          kind="warn"
          title={t('achado.bloqueados', { count: finding.mods.length })}
          actionsBelow
          actions={<ModList packId={packId} mods={finding.mods} />}
        >
          {t('achado.bloqueadosTexto')}
        </Alert>
      );
    case 'curseforgeOnly':
      return (
        <Alert
          kind="warn"
          title={t('achado.soCurseforge', { count: finding.mods.length })}
          actionsBelow
          actions={<ModList packId={packId} mods={finding.mods} />}
        />
      );
    case 'untested':
      return (
        <Alert
          kind="warn"
          title={t('achado.naoTestada', { version })}
          actions={
            <Link
              to="/packs/$packId/teste"
              params={{ packId }}
              className={buttonVariants({ size: 'sm' })}
              onClick={onClose}
            >
              <Icon icon={Play} />
              <span>{t('achado.testarAgora')}</span>
            </Link>
          }
        >
          {t('achado.naoTestadaTexto')}
        </Alert>
      );
    case 'checkFailed':
      return (
        <Alert kind="info" compact>
          {finding.reason === 'keyMissing' ? t('achado.semChave') : t('achado.semConexao')}
        </Alert>
      );
    case 'secretFound':
    case 'hygiene':
      return null;
  }
}

/** A lista de mods de um aviso da CurseForge, com "Trocar pelo Modrinth" onde existe. */
function ModList({ packId, mods }: { packId: PackId; mods: PublishMod[] }) {
  const { t } = useTranslation('publicar');
  const swap = useSwapToModrinth(packId);
  const [swapped, setSwapped] = useState<ReadonlySet<string>>(new Set());
  return (
    <ul className="publish__mods">
      {mods.map((mod) => {
        let detail: ReactNode;
        if (mod.swap === null) {
          detail = t('achado.naoExiste');
        } else if (mod.swap.sameFile) {
          detail = t('achado.mesmoArquivo');
        } else {
          detail = t('achado.outraVersao', {
            arquivo: mod.swap.fileName,
            versao: mod.swap.versionNumber,
          });
        }
        const done = swapped.has(mod.path);
        return (
          <li key={mod.path} className="publish__mod t-sm">
            <span>
              <b>{mod.name}</b>{' '}
              <span className="t-3">
                · {done ? t('achado.trocado', { nome: mod.name }) : detail}
              </span>
            </span>
            {mod.swap !== null && !done ? (
              <Button
                size="sm"
                aria-label={t('achado.trocarNome', { nome: mod.name })}
                loading={swap.isPending && swap.variables.items[0]?.replaces === mod.path}
                onClick={() => {
                  const target = mod.swap;
                  if (target === null) {
                    return;
                  }
                  swap.mutate(
                    {
                      items: [
                        {
                          source: 'modrinth',
                          projectId: target.projectId,
                          versionId: target.versionId,
                          side: target.side,
                          replaces: mod.path,
                        },
                      ],
                    },
                    {
                      onSuccess: () => {
                        setSwapped((current) => new Set([...current, mod.path]));
                        showToast({ kind: 'ok', title: t('achado.trocado', { nome: mod.name }) });
                      },
                      onError: (error) => {
                        showToast({
                          kind: 'danger',
                          title: t('achado.trocaFalhou', { nome: mod.name }),
                          text: appErrorMessage(toAppError(error)),
                        });
                      },
                    },
                  );
                }}
              >
                {t('achado.trocar')}
              </Button>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

function ResultContent({
  packId,
  published,
  returnFocus,
  onClose,
}: {
  packId: PackId;
  published: Published;
  returnFocus: () => HTMLElement | null;
  onClose: () => void;
}) {
  const { t } = useTranslation('publicar');
  const status = useGithubStatus(packId);
  const linkId = useId();
  const isPrivate = status.data?.link?.private ?? false;
  const game = {
    minecraft: status.data?.minecraft ?? null,
    loader: status.data?.loader ?? null,
  };
  const Prism = githubSlots.prismInstance;
  return (
    <DialogContent
      title={t('resultado.titulo', { version: published.version })}
      description={t('resultado.descricao')}
      size="lg"
      returnFocusTo={returnFocus}
      footer={
        <Button variant="primary" onClick={onClose}>
          {t('resultado.fechar')}
        </Button>
      }
    >
      <div className="stack-3">
        {isPrivate ? (
          <Alert kind="info" compact>
            {t('area.soBackup')}
          </Alert>
        ) : (
          <>
            <div className="field">
              <label className="field__label" htmlFor={linkId}>
                {t('area.link')}
              </label>
              <div className="linkbox">
                <input id={linkId} className="input" readOnly value={published.packLink} />
                <Button
                  variant="primary"
                  icon={Copy}
                  onClick={() => {
                    void copyText(t, published.packLink, 'copiar.copiado');
                  }}
                >
                  {t('copiar.link')}
                </Button>
              </div>
            </div>
            <PlayerSteps packLink={published.packLink} game={game} />
          </>
        )}
        {Prism ? <Prism packId={packId} published={published} /> : null}
        <div className="btn-row">
          <Button
            size="sm"
            icon={Copy}
            onClick={() => {
              void copyText(t, published.notes, 'copiar.notasCopiadas');
            }}
          >
            {t('copiar.notas')}
          </Button>
          <Button
            size="sm"
            icon={ExternalLink}
            onClick={() => {
              void openExternal(published.release.htmlUrl);
            }}
          >
            {t('resultado.abrirRelease')}
          </Button>
        </div>
      </div>
    </DialogContent>
  );
}
