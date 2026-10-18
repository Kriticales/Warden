/**
 * A parte "Desempenho" do resultado do teste (SPEC T13 passo 7; ROADMAP L-12; protótipo
 * `teste-fechou~desempenho`): "O que mais demorou para carregar" (o tempo por mod lido do log do
 * loader, com a ressalva do que não entra na medida) e o resumo de cada perfil do spark salvo no
 * teste (tempo por tick e os mods que mais usaram processamento ou memória, agrupados pelo jar),
 * com "Abrir no visualizador do spark". Registrado em `../result-blocks.ts`; só aparece nas
 * sessões com perfil de desempenho.
 */
import { ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import i18n from '../../../i18n';
import { formatTime } from '../../../lib/format';
import type {
  LoadReport,
  PackId,
  PerfReport as PerfReportData,
  SparkReport,
} from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import type { ResultBlockProps } from '../result-blocks';
import { formatDuration } from '../text';
import { useOpenSparkViewer, usePerfReport } from './api';
import { loadMsText, msptText, percentText, tpsText } from './format';
import './perf-report.css';

/** Quantos mods a tabela mostra. */
const SHOWN_MODS = 10;
/** Quantos mods o resumo do spark cita. */
const SHOWN_SOURCES = 3;

export function PerfReport({ packId, session }: ResultBlockProps) {
  // Só o perfil no jogo e o "como servidor" (com o perfil, quando pedido) gravam o campo.
  if (session.mode !== 'performance' && session.mode !== 'server') {
    return null;
  }
  return <PerfReportBody packId={packId} sessionId={session.id} />;
}

function PerfReportBody({ packId, sessionId }: { packId: PackId; sessionId: string }) {
  const { t } = useTranslation('perfilDesempenho');
  const report = usePerfReport(packId, sessionId);
  const open = useOpenSparkViewer(packId, sessionId);

  if (report.isPending) {
    return <LoadingState inline label={t('resultado.carregando')} />;
  }
  if (report.isError) {
    return <ErrorPanel compact error={report.error} />;
  }
  if (!report.data) {
    return null;
  }
  return (
    <PerfReportPanel
      report={report.data}
      opening={open.isPending ? open.variables : null}
      onOpenSpark={(file) => {
        open.mutate(file, {
          onError: (error) => {
            showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
          },
        });
      }}
    />
  );
}

/** O perfil gravado (sem consulta: também usado nos testes de componente). */
export function PerfReportPanel({
  report,
  opening,
  onOpenSpark,
}: {
  report: PerfReportData;
  /** O arquivo sendo aberto no visualizador, se algum. */
  opening: string | null;
  onOpenSpark: (file: string) => void;
}) {
  const { t } = useTranslation('perfilDesempenho');
  const server = report.side === 'server';
  // O mais novo primeiro.
  const files = [...report.spark].reverse();
  return (
    <section className="panel panel--strong perf-report" aria-labelledby="perf-report-title">
      <div className="panel__head">
        <div>
          <h2 className="panel__title" id="perf-report-title">
            {t('resultado.titulo')}
          </h2>
          <p className="t-sm t-3">{server ? t('resultado.subServidor') : t('resultado.subJogo')}</p>
        </div>
      </div>
      <div className="grid-2 perf-report__grid">
        <div>
          <h3 className="field__label">{t('resultado.carregamento.titulo')}</h3>
          {report.load ? (
            <LoadTimes load={report.load} />
          ) : (
            <p className="t-sm t-2 perf-report__gap">{t('resultado.carregamento.servidor')}</p>
          )}
        </div>
        <div className="stack-3">
          {files.length === 0 ? <NoSpark report={report} /> : null}
          {files.map((file) => (
            <SparkSummary
              key={file.file}
              file={file}
              server={server}
              opening={opening === file.file}
              onOpen={() => {
                onOpenSpark(file.file);
              }}
            />
          ))}
          {report.sparkSkipped > 0 ? (
            <p className="t-xs t-3">
              {t('resultado.spark.deFora', { count: report.sparkSkipped })}
            </p>
          ) : null}
        </div>
      </div>
    </section>
  );
}

/** O nome do loader para as frases. */
function loaderName(loader: string): string {
  switch (loader) {
    case 'neoforge':
      return 'NeoForge';
    case 'fabric':
      return 'Fabric';
    case 'quilt':
      return 'Quilt';
    default:
      return 'Forge';
  }
}

function LoadTimes({ load }: { load: LoadReport }) {
  const { t } = useTranslation('perfilDesempenho');
  const loader = loaderName(load.loader);
  if (load.status === 'notRecorded') {
    const text =
      load.loader === 'fabric'
        ? t('resultado.carregamento.semDadoFabric')
        : load.loader === 'quilt'
          ? t('resultado.carregamento.semDadoQuilt')
          : t('resultado.carregamento.semDadoVanilla');
    return <p className="t-sm t-2 perf-report__gap">{text}</p>;
  }
  if (load.status === 'logMissing') {
    return (
      <p className="t-sm t-2 perf-report__gap">{t('resultado.carregamento.semLog', { loader })}</p>
    );
  }
  if (load.status === 'noLines') {
    return (
      <p className="t-sm t-2 perf-report__gap">
        {t('resultado.carregamento.semLinhas', { loader, log: load.log ?? '' })}
      </p>
    );
  }
  const shown = load.mods.slice(0, SHOWN_MODS);
  const more = load.mods.length - shown.length;
  return (
    <>
      <div className="tablewrap perf-report__gap">
        <table className="table table--plain" aria-label={t('resultado.carregamento.tabela')}>
          <thead>
            <tr>
              <th scope="col">{t('resultado.carregamento.mod')}</th>
              <th scope="col" className="num">
                {t('resultado.carregamento.tempo')}
              </th>
            </tr>
          </thead>
          <tbody>
            {shown.map((row) => (
              <tr key={row.key}>
                <td>{row.name}</td>
                <td className="num t-mono">{loadMsText(row.ms)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {more > 0 ? (
        <p className="t-xs t-3">{t('resultado.carregamento.maisMods', { count: more })}</p>
      ) : null}
      {load.gameAndLoaderMs > 0 ? (
        <p className="t-xs t-3">
          {t('resultado.carregamento.jogoELoader', {
            loader,
            tempo: loadMsText(load.gameAndLoaderMs),
          })}
        </p>
      ) : null}
      <p className="t-xs t-3 perf-report__gap">
        {t('resultado.carregamento.ressalva', { loader })}
      </p>
    </>
  );
}

function NoSpark({ report }: { report: PerfReportData }) {
  const { t } = useTranslation('perfilDesempenho');
  if (report.side === 'server') {
    const text =
      report.serverSpark === 'notInstalled'
        ? t('resultado.spark.semSparkServidor')
        : report.serverSpark === 'commandFailed'
          ? t('resultado.spark.falhouServidor')
          : t('resultado.spark.ligadoServidor');
    return <p className="t-sm t-2">{text}</p>;
  }
  return (
    <div>
      <p className="t-sm t-2">{t('resultado.spark.nenhum')}</p>
      <p className="t-xs t-3 perf-report__gap">{t('resultado.spark.comoSalvar')}</p>
    </div>
  );
}

function sparkTitle(
  file: SparkReport,
  server: boolean,
  t: ReturnType<typeof useTranslation<'perfilDesempenho'>>['t'],
): string {
  const hora =
    file.savedAtMs === null ? i18n.t('vazio', { ns: 'desempenho' }) : formatTime(file.savedAtMs);
  if (file.kind === 'heap') {
    return server
      ? t('resultado.spark.tituloMemoriaServidor', { hora })
      : t('resultado.spark.tituloMemoria', { hora });
  }
  return server
    ? t('resultado.spark.tituloPerfilServidor', { hora })
    : t('resultado.spark.tituloPerfil', { hora });
}

function SparkSummary({
  file,
  server,
  opening,
  onOpen,
}: {
  file: SparkReport;
  server: boolean;
  opening: boolean;
  onOpen: () => void;
}) {
  const { t } = useTranslation('perfilDesempenho');
  const top = file.top.slice(0, SHOWN_SOURCES);
  const shares = top
    .map((row) =>
      t('resultado.spark.parte', { nome: row.name, porcentagem: percentText(row.percent) }),
    )
    .join(', ');
  return (
    <div>
      <h3 className="field__label">{sparkTitle(file, server, t)}</h3>
      {file.readable ? (
        <dl className="kv perf-report__gap">
          {file.durationMs === null ? null : (
            <>
              <dt>{t('resultado.spark.duracao')}</dt>
              <dd>{formatDuration(file.durationMs)}</dd>
            </>
          )}
          {file.ticks ? (
            <>
              <dt>{t('resultado.spark.tick')}</dt>
              <dd>
                {t('resultado.spark.tickValor', {
                  mediana: msptText(file.ticks.msptMedian),
                  pior: msptText(file.ticks.msptMax),
                })}
              </dd>
              {file.ticks.tps === null ? null : (
                <>
                  <dt>{t('resultado.spark.tps')}</dt>
                  <dd>{t('resultado.spark.tpsValor', { tps: tpsText(file.ticks.tps) })}</dd>
                </>
              )}
            </>
          ) : null}
          <dt>
            {file.kind === 'heap'
              ? t('resultado.spark.memoria')
              : t('resultado.spark.processamento')}
          </dt>
          <dd>
            {top.length > 0 ? shares : t('resultado.spark.soJogo')}
            {top.length > 0 ? (
              <span className="t-3">
                {' '}
                {t('resultado.spark.jogo', { porcentagem: percentText(file.gamePercent) })}
              </span>
            ) : null}
          </dd>
        </dl>
      ) : (
        <p className="t-sm t-warn perf-report__gap">{t('resultado.spark.ilegivel')}</p>
      )}
      <div className="row row--wrap perf-report__gap">
        <Button size="sm" icon={ExternalLink} disabled={opening} onClick={onOpen}>
          {opening ? t('resultado.spark.abrindo') : t('resultado.spark.abrir')}
        </Button>
      </div>
      <p className="t-xs t-3 perf-report__gap">{t('resultado.spark.abrirNota')}</p>
    </div>
  );
}
