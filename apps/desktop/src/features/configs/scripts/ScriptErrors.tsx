/**
 * Painel "Erros dos scripts" (SPEC T26; protótipo `.scripterr`): os erros e avisos do último
 * teste (logs da instância de teste) ou, com o KubeJS 7 e o jogo aberto, ao vivo. Cada item
 * mostra o arquivo e a linha (clicar leva à linha; em outro arquivo, abre o arquivo nela), a
 * mensagem e a linha original do log.
 */
import { CircleAlert } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { dayRelation, formatDate, formatTime } from '../../../lib/format';
import type { ScriptProblem, ScriptProblems } from '../../../lib/ipc/bindings';
import type { EditorAddonApi } from '../components/ConfigEditor';
import { fileName } from './model';
import { setPendingLine, takePendingLine } from './pending';

export interface ScriptErrorsProps {
  api: EditorAddonApi;
  /** O arquivo aberto. */
  path: string;
  /** Os erros da ferramenta do arquivo, os do arquivo aberto primeiro. */
  problems: readonly ScriptProblem[];
  data: ScriptProblems | undefined;
  loading: boolean;
  error: unknown;
  onRetry: () => void;
  /** Abre outro script. */
  onOpenFile: (path: string) => void;
}

function useWhen(data: ScriptProblems | undefined): string | null {
  const { t } = useTranslation('scripts');
  const [now] = useState(() => Date.now());
  const latest = Math.max(0, ...(data?.logs ?? []).map((log) => log.modifiedMs ?? 0));
  if (latest === 0) return null;
  const hora = formatTime(latest);
  switch (dayRelation(latest, now)) {
    case 'today':
      return t('erros.hoje', { hora });
    case 'yesterday':
      return t('erros.ontem', { hora });
    case 'other':
      return t('erros.outroDia', { data: formatDate(latest), hora });
  }
}

export function ScriptErrors({
  api,
  path,
  problems,
  data,
  loading,
  error,
  onRetry,
  onOpenFile,
}: ScriptErrorsProps) {
  const { t } = useTranslation('scripts');
  const titleId = useId();
  const when = useWhen(data);

  // Veio de um clique num erro de outro arquivo: vai à linha.
  useEffect(() => {
    const target = takePendingLine(path);
    if (target) {
      api.gotoLine(target.line, target.column);
    }
  }, [api, path]);

  const meta = data?.live
    ? t('erros.aoVivo')
    : when
      ? t('erros.meta', {
          quando: when,
          arquivos: (data?.logs ?? []).map((log) => log.file).join(', '),
        })
      : '';

  const go = (problem: ScriptProblem) => {
    if (!problem.file || !problem.line) return;
    if (problem.file.toLowerCase() === path.toLowerCase()) {
      api.gotoLine(problem.line, problem.column ?? 1);
    } else {
      setPendingLine(problem.file, problem.line, problem.column ?? 1);
      onOpenFile(problem.file);
    }
  };

  return (
    <section
      className="scripterr"
      aria-labelledby={titleId}
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- região rolável: o teclado precisa chegar nela
      tabIndex={0}
    >
      <div className="scripterr__head">
        <h2 className="scripterr__title" id={titleId}>
          <CircleAlert className="icon icon--sm" aria-hidden="true" />
          {t('erros.titulo')}
        </h2>
        <span className="t-xs t-3">{meta}</span>
      </div>
      {loading ? (
        <p className="scripterr__empty t-sm t-3">{t('erros.carregando')}</p>
      ) : error ? (
        <div className="scripterr__empty">
          <ErrorPanel title={t('erros.erro')} error={error} onRetry={onRetry} />
        </div>
      ) : problems.length > 0 ? (
        <ul>
          {problems.map((problem, index) => {
            const where =
              problem.file && problem.line
                ? problem.column
                  ? t('erros.localColuna', {
                      arquivo: fileName(problem.file),
                      linha: problem.line,
                      coluna: problem.column,
                    })
                  : t('erros.local', { arquivo: fileName(problem.file), linha: problem.line })
                : null;
            return (
              <li key={`${String(index)}:${problem.logLine}`}>
                {where ? (
                  <button
                    type="button"
                    className="scripterr__loc"
                    title={problem.file ?? undefined}
                    onClick={() => {
                      go(problem);
                    }}
                  >
                    {where}
                  </button>
                ) : (
                  <span className="scripterr__loc scripterr__loc--none">
                    {problem.level === 'warning' ? t('erros.aviso') : ''}
                  </span>
                )}
                <span className="scripterr__msg">
                  {problem.level === 'warning' && where ? (
                    <span className="tag tag--warn scripterr__level">{t('erros.aviso')}</span>
                  ) : null}
                  {problem.message}
                </span>
                <pre className="code">{problem.logLine}</pre>
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="scripterr__empty t-sm t-3">
          {(data?.logs.length ?? 0) > 0 || data?.live ? t('erros.vazio') : t('erros.semLogs')}
        </p>
      )}
    </section>
  );
}
