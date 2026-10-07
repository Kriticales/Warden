/**
 * "Instância pronta para o Prism" em Exportar (SPEC T19; ADR-0050): confere o que vai no
 * arquivo (mods bloqueados com **Trocar pelo Modrinth**, arquivos de terceiros com a confirmação
 * de licença) e gera o `.mrpack`. O destino é escolhido no diálogo nativo do Rust.
 */
import { FolderOpen, Package } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { formatBytes } from '../../../lib/format';
import { commandError } from '../../../lib/ipc/query';
import type {
  OperationSnapshot,
  PackId,
  PrismAnalysis,
  PrismResult,
  TestMemory,
} from '../../../lib/ipc/bindings';
import { Checkbox } from '../../settings/components/fields';
import { useCancelOperation } from '../api';
import { useTestSettings } from '../../pack-editor/api';
import { usePrismAnalysis, usePrismRun, useRevealPrism, useRunningPrism } from './api';
import { PrismSteps } from './PrismSteps';

/** A memória fixa do teste, em GB, para o passo a passo; sem valor fixo, o texto usa 6 GB. */
function fixedMemory(memory: TestMemory | undefined): string | undefined {
  if (memory?.mode !== 'fixed') return undefined;
  return `${String(Math.max(1, Math.round(memory.mb / 1024)))} GB`;
}

function isCancelled(error: unknown): boolean {
  const code = commandError(error)?.code;
  return code?.domain === 'core' && code.code === 'CANCELLED';
}

export function PrismPanel({ packId, readOnly }: { packId: PackId; readOnly: boolean }) {
  const { t } = useTranslation('instanciaPrism');
  const analysis = usePrismAnalysis(packId, true);
  const run = usePrismRun(packId);
  const running = useRunningPrism(packId);
  const settings = useTestSettings(packId);
  const memory = fixedMemory(settings.data?.settings.memory);
  const [swaps, setSwaps] = useState<readonly string[]>([]);
  const [confirmLocal, setConfirmLocal] = useState(false);
  const [result, setResult] = useState<PrismResult | null>(null);

  if (analysis.isPending) return <LoadingState label={t('carregando')} />;
  if (analysis.isError) {
    return (
      <ErrorPanel
        title={t('erroAnalise')}
        error={analysis.error}
        onRetry={() => {
          void analysis.refetch();
        }}
      />
    );
  }
  const data = analysis.data;
  const unresolved = data.blocked.filter((mod) => !swaps.includes(mod.path));
  const needsLocal = data.localFiles.length > 0 && !confirmLocal;
  const reason = blockReason(data, unresolved.length, needsLocal);
  const disabled = reason !== null;

  const start = () => {
    run.mutate(
      { confirmLocalFiles: confirmLocal, swaps: [...swaps] },
      {
        onSuccess: (value) => {
          if (value) setResult(value);
        },
      },
    );
  };

  return (
    <div className="stack">
      <section className="panel" aria-labelledby="prism-resumo">
        <h2 className="panel__title panel__title--sans" id="prism-resumo">
          {t('resumo.titulo')}
        </h2>
        <ul className="mt-3 t-sm">
          <li>
            {t('resumo.jogo', {
              minecraft: data.minecraft,
              loader: data.loader ? t('resumo.loader', { loader: data.loader }) : '',
              versao: data.version || '—',
            })}
          </li>
          <li>{t('resumo.modrinth', { count: data.modrinth })}</li>
          <li>{t('resumo.curseforge', { count: data.curseforge })}</li>
          {data.links > 0 ? <li>{t('resumo.links', { count: data.links })}</li> : null}
          <li>{t('resumo.overrides', { count: data.overrides })}</li>
          <li>{t('resumo.naoConfiaveis', { count: data.untrusted })}</li>
        </ul>
        <p className="mt-3 t-sm t-2">{t('explicacao.texto')}</p>
        <p className="mt-1 t-sm t-2">{t('resumo.faltandoJars')}</p>
      </section>

      {data.version === '' ? (
        <Alert kind="warn" title={t('semVersao.titulo')}>
          {t('semVersao.texto')}
        </Alert>
      ) : null}
      {data.curseforgeKeyMissing ? (
        <Alert kind="warn" title={t('semChave.titulo')}>
          {t('semChave.texto')}
        </Alert>
      ) : null}

      {data.blocked.length > 0 ? (
        <section className="panel" aria-labelledby="prism-bloqueados">
          <h2 className="panel__title panel__title--sans" id="prism-bloqueados">
            {t('bloqueados.titulo', { count: data.blocked.length })}
          </h2>
          <p className="mt-2 t-sm t-2">{t('bloqueados.texto')}</p>
          <ul className="mt-3 stack">
            {data.blocked.map((mod) => {
              const swapped = swaps.includes(mod.path);
              return (
                <li key={mod.path} className="row">
                  <span className="grow">
                    <strong>{mod.name}</strong> <span className="t-3">{mod.fileName}</span>
                    <span className="block t-sm t-2">
                      {swapped && mod.swap
                        ? t('bloqueados.trocado', {
                            arquivo: mod.swap.fileName,
                            versao: mod.swap.versionNumber,
                          })
                        : mod.swap
                          ? null
                          : t('bloqueados.semTroca')}
                    </span>
                  </span>
                  {mod.swap ? (
                    <Button
                      size="sm"
                      variant={swapped ? 'ghost' : 'secondary'}
                      aria-label={
                        swapped
                          ? t('bloqueados.desfazer', { nome: mod.name })
                          : t('bloqueados.trocarNome', { nome: mod.name })
                      }
                      onClick={() => {
                        setSwaps((current) =>
                          swapped ? current.filter((p) => p !== mod.path) : [...current, mod.path],
                        );
                      }}
                    >
                      {swapped ? t('bloqueados.desfazerCurto') : t('bloqueados.trocar')}
                    </Button>
                  ) : null}
                </li>
              );
            })}
          </ul>
        </section>
      ) : null}

      {data.localFiles.length > 0 ? (
        <section className="panel" aria-labelledby="prism-locais">
          <h2 className="panel__title panel__title--sans" id="prism-locais">
            {t('locais.titulo', { count: data.localFiles.length })}
          </h2>
          <p className="mt-2 t-sm t-2">{t('locais.texto')}</p>
          <ul className="mt-2 t-sm">
            {data.localFiles.map((file) => (
              <li key={file.path}>
                <span className="path">{file.path}</span>{' '}
                <span className="t-3">{formatBytes(file.bytes)}</span>
              </li>
            ))}
          </ul>
          <div className="mt-3">
            <Checkbox
              label={t('locais.confirmar')}
              desc={t('locais.confirmarDesc')}
              checked={confirmLocal}
              onChange={setConfirmLocal}
            />
          </div>
        </section>
      ) : null}

      {run.isError && !isCancelled(run.error) ? (
        <ErrorPanel title={t('acao.falha')} error={run.error} />
      ) : null}
      {running ? (
        <RunningPanel operation={running} />
      ) : (
        <div className="export-foot">
          <p className="grow t-sm t-2">
            {reason ?? t('acao.dica')}
            {readOnly ? <span className="t-3 block">{t('acao.somenteLeitura')}</span> : null}
          </p>
          <Button
            variant="primary"
            size="lg"
            icon={Package}
            disabled={disabled}
            loading={run.isPending}
            onClick={start}
          >
            {t('acao.gerar')}
          </Button>
        </div>
      )}
      <Dialog
        open={result !== null}
        onOpenChange={(open) => {
          if (!open) setResult(null);
        }}
      >
        {result ? <ResultDialog result={result} memory={memory} /> : null}
      </Dialog>
    </div>
  );

  function blockReason(value: PrismAnalysis, blocked: number, local: boolean): string | null {
    if (value.version === '') return t('semVersao.texto');
    if (value.curseforgeKeyMissing) return t('semChave.texto');
    if (blocked > 0 || local) return t('acao.bloqueadoMotivo');
    return null;
  }
}

function RunningPanel({ operation }: { operation: OperationSnapshot }) {
  const { t } = useTranslation('instanciaPrism');
  const cancel = useCancelOperation();
  const waiting = operation.state === 'waitingForLock';
  const current = operation.progress;
  const value =
    current && current.total !== null && current.total > 0
      ? (current.current / current.total) * 100
      : null;
  return (
    <div className="panel">
      <ProgressBar
        value={value}
        label={t('andamento.rotulo')}
        meta={waiting ? t('andamento.aguardando') : t('andamento.meta')}
        state={waiting ? 'waiting' : 'running'}
      />
      {operation.cancellable ? (
        <Button
          variant="ghost"
          size="sm"
          className="mt-3"
          loading={cancel.isPending || operation.state === 'cancelling'}
          onClick={() => {
            cancel.mutate(operation.id);
          }}
        >
          {t('andamento.cancelar')}
        </Button>
      ) : null}
    </div>
  );
}

function ResultDialog({ result, memory }: { result: PrismResult; memory: string | undefined }) {
  const { t } = useTranslation('instanciaPrism');
  const { t: tExport } = useTranslation('exportar');
  const reveal = useRevealPrism();
  return (
    <DialogContent
      size="sm"
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
        {t('resultado.resumo', {
          arquivos: tExport('conteudo.arquivos', { count: result.files }),
          overrides: tExport('conteudo.arquivos', { count: result.overrides }),
          tamanho: formatBytes(result.bytes),
        })}{' '}
        <span className="path">{result.path}</span>.
      </p>
      <p className="mt-2 t-sm">
        {t('resultado.sha')}: <code className="path">{result.sha256}</code>
      </p>
      <Alert kind="ok" compact title={t('resultado.conferido')} className="mt-3">
        {t('resultado.conferidoTexto')}
      </Alert>
      <div className="mt-3">
        <PrismSteps memory={memory} />
      </div>
      {reveal.isError ? <ErrorPanel compact error={reveal.error} className="mt-3" /> : null}
    </DialogContent>
  );
}
