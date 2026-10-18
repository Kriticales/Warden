/**
 * "Pacote para servidor (.zip)" em Exportar (SPEC T19; ADR-0035; protótipo
 * `exportar-servidor`): como os mods chegam ao servidor (pelo link do pack, o padrão, ou dentro
 * do zip), a memória, o que vai no pacote e o que fica de fora, o aviso de pacote não testado
 * com **Testar como servidor**, a caixa "Testar no servidor deste computador depois de gerar"
 * (marcada quando a EULA já foi aceita; marcar sem o aceite pergunta a EULA antes) e o resultado.
 *
 * O destino é escolhido no diálogo nativo do Rust. Com mods dentro, os mods bloqueados da
 * CurseForge abrem o download manual (T20) aqui mesmo, enquanto a geração espera.
 */
import { useNavigate } from '@tanstack/react-router';
import { ExternalLink, FolderOpen, Server } from 'lucide-react';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import { openExternal } from '../../../lib/external';
import { formatBytes } from '../../../lib/format';
import type {
  OperationSnapshot,
  PackId,
  PackRow,
  ServerPackAnalysis,
  ServerPackInfo,
  ServerPackRun,
  ServerPackVariant,
} from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { commandError } from '../../../lib/ipc/query';
import { Checkbox, Choice, SelectField } from '../../settings/components/fields';
import { useSettings } from '../../settings/api';
import { ManualDownloads, useManualDownloads } from '../../test/blocked/ManualDownloads';
import { WhyServerCrashed } from '../../test/server/ServerResult';
import { ServerTestDialog } from '../../test/server/ServerTestDialog';
import { useAcceptEula } from '../../test/server/api';
import { EULA_URL, clientOnlyText } from '../../test/server/text';
import { formatDuration, formatMemory } from '../../test/text';
import { useCancelOperation } from '../api';
import {
  useRevealServerPack,
  useRunningServerPack,
  useServerPackInfo,
  useServerPackRun,
} from './api';
import { memoryArg, memoryOptions } from './model';

/** O grupo de rádio "Como os mods chegam ao servidor". */
const RADIO_NAME = 'server-pack-variant';

function isCancelled(error: unknown): boolean {
  const code = commandError(error)?.code;
  return code?.domain === 'core' && code.code === 'CANCELLED';
}

/** A variante escolhida, ou a padrão: pelo link quando o pack está publicado. */
function pickVariant(info: ServerPackInfo, chosen: ServerPackVariant | null): ServerPackVariant {
  if (chosen === 'link' && info.published) return 'link';
  if (chosen === 'embedded') return 'embedded';
  return info.published ? 'link' : 'embedded';
}

export function ServerPackPanel({
  packId,
  pack,
  readOnly,
}: {
  packId: PackId;
  pack: PackRow | undefined;
  readOnly: boolean;
}) {
  const { t } = useTranslation('pacoteServidor');
  const info = useServerPackInfo(packId);
  const settings = useSettings();
  const run = useServerPackRun(packId);
  const running = useRunningServerPack(packId);
  const [chosen, setChosen] = useState<ServerPackVariant | null>(null);
  const [memory, setMemory] = useState<number | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [validate, setValidate] = useState<boolean | null>(null);
  const [eulaOpen, setEulaOpen] = useState(false);
  const [serverTestOpen, setServerTestOpen] = useState(false);
  const [result, setResult] = useState<ServerPackRun | null>(null);
  const testButton = useRef<HTMLButtonElement>(null);
  const validateBox = useRef<HTMLDivElement>(null);

  if (info.isPending) return <LoadingState label={t('carregando')} />;
  if (info.isError) {
    return (
      <ErrorPanel
        title={t('erroAnalise')}
        error={info.error}
        onRetry={() => {
          void info.refetch();
        }}
      />
    );
  }
  const data = info.data;
  const variant = pickVariant(data, chosen);
  const analysis = variant === 'link' && data.published ? data.published.analysis : data.current;
  const contents = variant === 'link' ? analysis.link : analysis.embedded;
  const memoryMb = memory ?? data.recommendedMemoryMb;
  // O aceite da EULA vem das Configurações (aceitar e revogar atualizam o cache delas).
  const eulaAccepted =
    (settings.data ? settings.data.eulaAcceptedAt : data.eulaAcceptedAt) !== null;
  const validating = (validate ?? eulaAccepted) && eulaAccepted;
  const needsConfirm = variant === 'embedded' && contents.thirdParty > 0 && !confirm;
  const keyMissing = variant === 'embedded' && data.current.curseforgeKeyMissing;
  const reason = !analysis.loaderSupported
    ? t('loaderNao', { loader: analysis.loader })
    : keyMissing
      ? t('semChave.texto')
      : needsConfirm
        ? t('acao.confirmar')
        : null;

  const start = () => {
    run.mutate(
      {
        options: { variant, memoryMb, confirmThirdParty: variant === 'embedded' && confirm },
        validate: validating,
      },
      {
        onSuccess: (value) => {
          if (value) setResult(value);
        },
      },
    );
  };

  return (
    <div className="stack">
      <section className="panel" aria-labelledby="sp-como">
        <h2 className="panel__title panel__title--sans" id="sp-como">
          {t('como.titulo')}
        </h2>
        <fieldset className="m-0 mt-3 border-0 p-0" aria-labelledby="sp-como">
          <div className="choice-list choice-list--2">
            <Choice
              name={RADIO_NAME}
              title={t('como.link.titulo')}
              badge={<span className="tag tag--ok ml-2">{t('como.recomendado')}</span>}
              desc={linkDesc(data)}
              checked={variant === 'link'}
              disabled={!data.published}
              onSelect={() => {
                setChosen('link');
              }}
            />
            <Choice
              name={RADIO_NAME}
              title={t('como.dentro.titulo')}
              desc={
                data.current.manualDownloads.length > 0
                  ? t('como.dentro.manual', { count: data.current.manualDownloads.length })
                  : t('como.dentro.desc')
              }
              checked={variant === 'embedded'}
              onSelect={() => {
                setChosen('embedded');
              }}
            />
          </div>
        </fieldset>
        {data.published ? null : <PublishLink packId={packId} />}
        <div className="row row--wrap mt-3">
          <SelectField
            label={t('como.memoria')}
            value={String(memoryMb)}
            options={memoryOptions(data.recommendedMemoryMb).map((mb) => ({
              value: String(mb),
              label:
                mb === data.recommendedMemoryMb
                  ? t('como.memoriaRecomendada', { memoria: formatMemory(mb) })
                  : formatMemory(mb),
            }))}
            onChange={(event) => {
              setMemory(Number(event.target.value));
            }}
          />
        </div>
      </section>

      <ContentsPanel analysis={analysis} variant={variant} memoryMb={memoryMb} />

      {!analysis.loaderSupported ? (
        <Alert kind="danger" title={t('loaderNao', { loader: analysis.loader })} />
      ) : null}
      {keyMissing ? (
        <Alert kind="warn" title={t('semChave.titulo')}>
          {t('semChave.texto')}
        </Alert>
      ) : null}
      {variant === 'embedded' && data.current.manualDownloads.length > 0 ? (
        <Alert
          kind="warn"
          title={t('manual.titulo', { count: data.current.manualDownloads.length })}
        >
          {t('manual.texto', { nomes: data.current.manualDownloads.join(', ') })}
        </Alert>
      ) : null}
      {variant === 'embedded' && contents.thirdParty > 0 ? (
        <section className="panel" aria-labelledby="sp-licenca">
          <h2 className="panel__title panel__title--sans" id="sp-licenca">
            {t('licenca.titulo', { count: contents.thirdParty })}
          </h2>
          <p className="mt-2 t-sm t-2">{t('licenca.texto')}</p>
          <div className="mt-3">
            <Checkbox
              label={t('licenca.confirmar')}
              desc={t('licenca.confirmarDesc')}
              checked={confirm}
              onChange={setConfirm}
            />
          </div>
        </section>
      ) : null}

      <Alert
        kind="warn"
        title={t('naoTestado.titulo')}
        actions={
          <Button
            ref={testButton}
            size="sm"
            icon={Server}
            onClick={() => {
              setServerTestOpen(true);
            }}
          >
            {t('naoTestado.testar')}
          </Button>
        }
      >
        {t('naoTestado.texto')}
      </Alert>
      <div ref={validateBox}>
        <Checkbox
          label={t('validar.rotulo')}
          desc={t('validar.desc')}
          checked={validating}
          onChange={(checked) => {
            if (checked && !eulaAccepted) {
              setEulaOpen(true);
              return;
            }
            setValidate(checked);
          }}
        />
      </div>

      {run.isError && !isCancelled(run.error) ? (
        <ErrorPanel title={t('acao.falha')} error={run.error} />
      ) : null}
      {running ? (
        <RunningPanel packId={packId} operation={running} />
      ) : (
        <div className="export-foot">
          <p className="grow t-sm t-2">
            {reason ?? t('acao.dica')}
            {readOnly ? <span className="t-3 block">{t('acao.somenteLeitura')}</span> : null}
          </p>
          <Button
            variant="primary"
            size="lg"
            icon={Server}
            disabled={reason !== null}
            loading={run.isPending}
            onClick={start}
          >
            {t('acao.gerar')}
          </Button>
        </div>
      )}

      <Dialog open={eulaOpen} onOpenChange={setEulaOpen}>
        {eulaOpen ? (
          <EulaDialog
            returnFocusTo={() => validateBox.current?.querySelector('input') ?? null}
            onAccepted={() => {
              setValidate(true);
              setEulaOpen(false);
            }}
            onClose={() => {
              setEulaOpen(false);
            }}
          />
        ) : null}
      </Dialog>
      {pack ? (
        <ServerTestDialog
          packId={packId}
          pack={pack}
          open={serverTestOpen}
          onOpenChange={setServerTestOpen}
          returnFocusTo={() => testButton.current}
        />
      ) : null}
      <Dialog
        open={result !== null}
        onOpenChange={(open) => {
          if (!open) setResult(null);
        }}
      >
        {result ? <ResultDialog run={result} loader={analysis.loader} /> : null}
      </Dialog>
    </div>
  );

  function linkDesc(value: ServerPackInfo): string {
    if (value.published) return t('como.link.desc', { versao: value.published.version });
    return value.linkUnavailable === 'privateRepo'
      ? t('como.link.privado')
      : t('como.link.semPublicacao');
  }
}

/** "Publicar versão": a área "Publicação para os jogadores" fica no Histórico. */
function PublishLink({ packId }: { packId: PackId }) {
  const { t } = useTranslation('pacoteServidor');
  const navigate = useNavigate();
  return (
    <p className="mt-2 t-sm">
      <Button
        variant="link"
        onClick={() => {
          void navigate({ to: '/packs/$packId/historico', params: { packId } });
        }}
      >
        {t('como.link.publicar')}
      </Button>
    </p>
  );
}

function ContentsPanel({
  analysis,
  variant,
  memoryMb,
}: {
  analysis: ServerPackAnalysis;
  variant: ServerPackVariant;
  memoryMb: number;
}) {
  const { t } = useTranslation('pacoteServidor');
  const contents = variant === 'link' ? analysis.link : analysis.embedded;
  const loader = analysis.loader || t('servidorMinecraft');
  const valor = memoryArg(memoryMb);
  return (
    <section className="panel" aria-labelledby="sp-conteudo">
      <h2 className="panel__title panel__title--sans" id="sp-conteudo">
        {t('conteudo.titulo')}
      </h2>
      <ul className="mt-3 t-sm stack-1">
        <li>{t('conteudo.scripts', { loader })}</li>
        <li>{t('conteudo.memoria', { valor })}</li>
        {variant === 'link' ? <li>{t('conteudo.bootstrap')}</li> : null}
        <li>
          {variant === 'link'
            ? t('conteudo.itensLink', { count: contents.serverItems })
            : t('conteudo.itensDentro', { count: contents.serverItems })}
        </li>
        {contents.folders.map((folder) => (
          <li key={folder.path}>
            {folder.path === '.'
              ? t('conteudo.raiz', { count: folder.files })
              : t('conteudo.pasta', { pasta: folder.path, count: folder.files })}
            {folder.path === 'kubejs' && analysis.kubejsClientScripts
              ? ` ${t('conteudo.semClientScripts')}`
              : null}
          </li>
        ))}
        {analysis.serverOverrides > 0 ? (
          <li>
            {variant === 'link'
              ? t('conteudo.overridesLink')
              : t('conteudo.overridesDentro', { count: analysis.serverOverrides })}
          </li>
        ) : null}
      </ul>
      <Alert
        kind="neutral"
        compact
        title={clientOnlyText(analysis.clientOnly, analysis.clientOnlyCount)}
        className="mt-3"
      >
        {t('conteudo.tambemDeFora')}
      </Alert>
      <p className="mt-3 t-sm t-2">{t('conteudo.eula')}</p>
      {variant === 'link' ? <p className="mt-1 t-sm t-2">{t('conteudo.versaoNova')}</p> : null}
    </section>
  );
}

/** "Aceitar a EULA do Minecraft?", com os textos do servidor local (D19). */
function EulaDialog({
  returnFocusTo,
  onAccepted,
  onClose,
}: {
  returnFocusTo: () => HTMLElement | null;
  onAccepted: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation('servidor');
  const accept = useAcceptEula();
  return (
    <DialogContent
      size="sm"
      title={t('eula.titulo')}
      returnFocusTo={returnFocusTo}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={accept.isPending}>
            {t('eula.agoraNao')}
          </Button>
          <Button
            variant="primary"
            loading={accept.isPending}
            onClick={() => {
              accept.mutate(undefined, {
                onSuccess: onAccepted,
                onError: (error) => {
                  showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
                },
              });
            }}
          >
            {accept.isPending ? t('eula.aceitando') : t('eula.aceitar')}
          </Button>
        </>
      }
    >
      <p>{t('eula.texto')}</p>
      <p>
        <Button
          variant="link"
          iconEnd={ExternalLink}
          onClick={() => {
            void openExternal(EULA_URL);
          }}
        >
          {t('eula.ler')}
        </Button>
      </p>
      <p className="t-sm t-2">{t('eula.lembrar')}</p>
    </DialogContent>
  );
}

function RunningPanel({ packId, operation }: { packId: PackId; operation: OperationSnapshot }) {
  const { t } = useTranslation('pacoteServidor');
  const cancel = useCancelOperation();
  const manual = useManualDownloads(packId);
  const waiting = operation.state === 'waitingForLock';
  const current = operation.progress;
  const value =
    current && current.total !== null && current.total > 0
      ? (current.current / current.total) * 100
      : null;
  return (
    <div className="panel stack">
      <ProgressBar
        value={value}
        label={t('andamento.rotulo')}
        meta={waiting ? t('andamento.aguardando') : t('andamento.meta')}
        state={waiting ? 'waiting' : 'running'}
      />
      {manual.data && manual.data.length > 0 ? (
        <ManualDownloads packId={packId} items={manual.data} />
      ) : null}
      {operation.cancellable ? (
        <div>
          <Button
            variant="ghost"
            size="sm"
            loading={cancel.isPending || operation.state === 'cancelling'}
            onClick={() => {
              cancel.mutate(operation.id);
            }}
          >
            {t('andamento.cancelar')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function ResultDialog({ run, loader }: { run: ServerPackRun; loader: string }) {
  const { t } = useTranslation('pacoteServidor');
  const reveal = useRevealServerPack();
  const { result, check } = run;
  return (
    <DialogContent
      title={t('resultado.titulo')}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('resultado.fechar')}</Button>
          </DialogClose>
          <Button
            variant="primary"
            icon={FolderOpen}
            loading={reveal.isPending}
            onClick={() => {
              reveal.mutate(result.path);
            }}
          >
            {t('resultado.mostrar')}
          </Button>
        </>
      }
    >
      <p>
        {t('resultado.resumo', { count: result.files.length, tamanho: formatBytes(result.bytes) })}{' '}
        <span className="path">{result.path}</span>.
      </p>
      {result.mods > 0 ? (
        <p className="mt-1">{t('resultado.mods', { count: result.mods })}</p>
      ) : null}
      <p className="mt-2 t-sm">
        {t('resultado.sha')}: <code className="path">{result.sha256}</code>
      </p>
      <div className="mt-3">
        {check === null ? (
          <p className="t-sm t-2">{t('resultado.naoTestado')}</p>
        ) : check.kind === 'tested' ? (
          <Alert
            kind="ok"
            compact
            title={t('resultado.testado', { tempo: formatDuration(check.readyMs) })}
          />
        ) : check.kind === 'crashed' ? (
          <Alert kind="danger" title={t('resultado.travou')}>
            <p>{t('resultado.travouTexto')}</p>
            <div className="mt-2">
              <WhyServerCrashed crash={check.crash} />
            </div>
          </Alert>
        ) : (
          <>
            <Alert kind="warn" compact title={t('resultado.naoTestou')} />
            <ErrorPanel compact error={check.error} className="mt-2" />
          </>
        )}
      </div>
      <section className="mt-3" aria-labelledby="sp-passos">
        <h3 className="t-strong" id="sp-passos">
          {t('resultado.passos.titulo')}
        </h3>
        <ol className="mt-2 t-sm stack-1" aria-label={t('resultado.passos.rotulo')}>
          <li>{t('resultado.passos.um')}</li>
          <li>{t('resultado.passos.dois')}</li>
          <li>{t('resultado.passos.tres', { loader: loader || t('servidorMinecraft') })}</li>
        </ol>
        {result.variant === 'link' ? (
          <p className="mt-2 t-sm t-2">{t('resultado.passos.link')}</p>
        ) : null}
      </section>
      {reveal.isError ? <ErrorPanel compact error={reveal.error} className="mt-3" /> : null}
    </DialogContent>
  );
}
