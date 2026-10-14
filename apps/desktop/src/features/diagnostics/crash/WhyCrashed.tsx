/**
 * "Por que travou" no resultado de um teste que travou (SPEC T13 passo 7 e T14; CA-T13-06;
 * protótipo `teste-travou`). Troca a parte provisória do Testar (`result-blocks.ts`):
 *
 * - a causa mais provável, com a correção sugerida e "Como sabemos" aberto (o trecho do log
 *   com o arquivo e a linha); as outras pistas ficam recolhidas;
 * - sem causa conhecida: "Não encontramos a causa automaticamente." e as últimas linhas com
 *   erro do console;
 * - "Abrir crash report", "Abrir erro do Java" e "Copiar relatório" (o texto passa pela
 *   redação antes de ir para a área de transferência).
 *
 * Tudo o que aparece já vem redigido do backend: nenhum caminho com o nome do usuário.
 */
import { Copy, FileText } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import i18n from '../../../i18n';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { CrashDiagnosis, TestSessionSummary } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { usePack } from '../../packs/api';
import { useOpenArtifact } from '../../test/api';
import { Console } from '../../test/console/Console';
import type { ResultBlockProps } from '../../test/result-blocks';
import { formatDuration, formatWhen } from '../../test/text';
import { useCrashDiagnosis, useRedact } from '../api';
import { FindingCard } from '../components/FindingCard';
import { useFixes } from '../fixes';
import { evidenceView, findingText, findingTitle } from '../text';
import '../diagnostics.css';

/** Quantas linhas com erro aparecem sem causa conhecida. */
const LAST_ERRORS = 12;

export function WhyCrashed({ packId, session, lines }: ResultBlockProps) {
  // Sem o jogo aberto ("Só o servidor", ou o servidor travou antes; L-09), o "Por que
  // travou" é o do servidor (`ServerResult`).
  if (session.outcome !== 'crashed' || session.server?.game === 'notOpened') {
    return null;
  }
  return <Crashed packId={packId} session={session} lines={lines} />;
}

function Crashed({ packId, session, lines }: ResultBlockProps) {
  const { t } = useTranslation('diagnostico');
  const { t: tt } = useTranslation('teste');
  const translate: Translate = (key, options) =>
    (i18n.t as unknown as Translate)(`diagnostico:${key}`, options);
  const translateTest: Translate = (key, options) =>
    (i18n.t as unknown as Translate)(`teste:${key}`, options);
  const diagnosis = useCrashDiagnosis(packId, session.id, true);
  const open = useOpenArtifact(packId);
  const redact = useRedact(packId);
  const fixes = useFixes(packId);
  const pack = usePack(packId);
  const duracao = formatDuration(session.durationMs);
  const errors = lines
    .filter((line) => line.level === 'error' || line.level === 'fatal')
    .slice(-LAST_ERRORS);

  const data = diagnosis.data;
  const [primary, ...others] = data?.foundCause ? data.findings : [];
  const showLines = !diagnosis.isPending && !primary;

  const openFile = (path: string) => {
    open.mutate(
      { sessionId: session.id, path },
      {
        onError: (error) => {
          showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
        },
      },
    );
  };
  const copy = () => {
    redact.mutate(reportText(pack.data?.name ?? '', session, data, translate, translateTest), {
      onSuccess: (text) => {
        navigator.clipboard.writeText(text).then(
          () => {
            showToast({ kind: 'ok', title: t('travou.copiado') });
          },
          () => {
            showToast({ kind: 'danger', title: t('travou.copiarFalhou') });
          },
        );
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };

  return (
    <section className="panel panel--strong stack-3" aria-labelledby="why-crashed-title">
      <h2 className="panel__title" id="why-crashed-title">
        {tt('porQueTravou.titulo')}
      </h2>
      <p>
        {session.exitCode === null
          ? tt('porQueTravou.semCodigo', { duracao })
          : tt('porQueTravou.codigo', { codigo: session.exitCode, duracao })}
      </p>
      {diagnosis.isPending ? <LoadingState label={t('travou.analisando')} /> : null}
      {diagnosis.isError ? (
        <Alert kind="warn" compact title={t('travou.erroAnalise')}>
          {appErrorMessage(toAppError(diagnosis.error))}
        </Alert>
      ) : null}
      {primary ? (
        <FindingCard
          packId={packId}
          entry={primary}
          evidenceOpen
          onFix={fixes.run}
          busyFix={fixes.busy}
        />
      ) : null}
      {data && !primary ? (
        <Alert kind="neutral" title={t('travou.semCausaTitulo')}>
          {t('travou.semCausaTexto')}
        </Alert>
      ) : null}
      {others.length > 0 ? (
        <details className="disclosure">
          <summary>{t('travou.outrasPistas', { count: others.length })}</summary>
          <div className="stack-2 diag-others">
            {others.map((entry) => (
              <FindingCard
                key={entry.id}
                packId={packId}
                entry={entry}
                headingLevel={4}
                onFix={fixes.run}
                busyFix={fixes.busy}
              />
            ))}
          </div>
        </details>
      ) : null}
      {session.crashReports.length > 0 ? (
        <p className="t-sm t-2">{tt('porQueTravou.comRelatorio')}</p>
      ) : null}
      {session.hsErrFiles.length > 0 ? (
        <p className="t-sm t-2">{tt('porQueTravou.comErroJava')}</p>
      ) : null}
      <div className="btn-row">
        {session.crashReports.map((path) => (
          <Button
            key={path}
            size="sm"
            variant={primary ? 'secondary' : 'primary'}
            icon={FileText}
            onClick={() => {
              openFile(path);
            }}
          >
            {tt('resultado.abrirCrash')}
          </Button>
        ))}
        {session.hsErrFiles.map((path) => (
          <Button
            key={path}
            size="sm"
            icon={FileText}
            onClick={() => {
              openFile(path);
            }}
          >
            {tt('resultado.abrirErroJava')}
          </Button>
        ))}
        <Button size="sm" variant="ghost" icon={Copy} loading={redact.isPending} onClick={copy}>
          {t('travou.copiar')}
        </Button>
      </div>
      {showLines ? (
        <div className="stack-2">
          <h3 className="field__label">{tt('porQueTravou.ultimasLinhas')}</h3>
          {errors.length > 0 ? (
            <Console
              lines={errors}
              state="ended"
              label={tt('porQueTravou.ultimasLinhas')}
              className="test-errors"
            />
          ) : (
            <p className="t-sm t-3">{tt('porQueTravou.semLinhas')}</p>
          )}
        </div>
      ) : null}
      {data && data.sources.length > 0 ? (
        <p className="t-xs t-3">
          {t('travou.lidos', { arquivos: data.sources.map((source) => source.name).join(', ') })}
        </p>
      ) : null}
      {fixes.dialogs}
    </section>
  );
}

type Translate = (key: string, options?: Record<string, unknown>) => string;

/** O texto do "Copiar relatório" (antes da redação, que o backend faz). */
export function reportText(
  packName: string,
  session: TestSessionSummary,
  diagnosis: CrashDiagnosis | undefined,
  t: Translate,
  tt: Translate,
): string {
  const lines = [
    t('relatorio.cabecalho'),
    t('relatorio.pack', { nome: packName }),
    t('relatorio.teste', {
      quando: formatWhen(session.startedAtMs),
      duracao: formatDuration(session.durationMs),
    }),
    t('relatorio.jogo', {
      jogo: [
        `Minecraft ${session.minecraft}`,
        session.loader ? `${session.loader} ${session.loaderVersion ?? ''}`.trim() : null,
      ]
        .filter(Boolean)
        .join(' · '),
      java: session.javaMajor,
    }),
    session.exitCode === null
      ? tt('porQueTravou.semCodigo', { duracao: formatDuration(session.durationMs) })
      : tt('porQueTravou.codigo', {
          codigo: session.exitCode,
          duracao: formatDuration(session.durationMs),
        }),
    '',
  ];
  const findings = diagnosis?.foundCause ? diagnosis.findings : [];
  if (findings.length === 0) {
    lines.push(t('relatorio.semCausa'));
  } else {
    lines.push(`${t('relatorio.causa')}:`);
    for (const entry of findings) {
      lines.push(`- ${findingTitle(entry)}`, `  ${findingText(entry)}`);
      const names = entry.items.map((item) => item.name);
      if (names.length > 0) lines.push(`  ${t('relatorio.itens', { nomes: names.join(', ') })}`);
      for (const evidence of entry.finding.evidence) {
        const view = evidenceView(entry, evidence);
        lines.push(`  ${t('relatorio.evidencia')}: ${view.label}`);
        if (view.excerpt) lines.push(`    ${view.excerpt.split('\n').join('\n    ')}`);
      }
    }
  }
  return lines.join('\n');
}
