/**
 * Diferença de um trecho (HANDOFF §3; DESIGN-SYSTEM §4): sinal + / − visível em cada linha e
 * "Adicionada:" / "Removida:" para o leitor de tela (nunca só a cor). Trechos iguais longos
 * ficam dobrados. Arquivos inteiros no editor usam o `@codemirror/merge`.
 *
 * Recebe as linhas prontas (`lines`) ou os dois textos (`before`/`after`), que passam por
 * `diffLines`.
 */
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { cn } from '../../lib/cn';
import { diffLines, diffStats, type DiffLine } from '../../lib/diff';

interface DiffBase {
  /** Caminho do arquivo, no cabeçalho. */
  file: string;
  className?: string | undefined;
}

export type DiffViewProps = DiffBase &
  (
    | { lines: readonly DiffLine[]; before?: never; after?: never; context?: never }
    | { before: string; after: string; context?: number | null; lines?: never }
  );

export function DiffView(props: DiffViewProps) {
  const { t } = useTranslation();
  const { file, className } = props;
  const before = props.lines ? null : props.before;
  const after = props.lines ? null : props.after;
  const context = props.lines ? undefined : props.context;
  const given = props.lines;
  const lines = useMemo(
    () => given ?? diffLines(before ?? '', after ?? '', context === undefined ? {} : { context }),
    [given, before, after, context],
  );
  const { added, removed } = diffStats(lines);
  return (
    <figure className={cn('diff', className)}>
      <figcaption className="diff__head">
        <span className="path">{file}</span>
        <span
          className="diff__stats"
          role="img"
          aria-label={t('diff.resumo', { adicionadas: added, removidas: removed })}
        >
          <span className="add">+{added}</span> <span className="del">−{removed}</span>
        </span>
      </figcaption>
      <div className="diff__body">
        {lines.map((line, index) => {
          if (line.kind === 'fold') {
            return (
              <div className="diff__fold" key={`fold-${String(index)}`}>
                {t('diff.oculto', { count: line.count })}
              </div>
            );
          }
          const oldNo = line.kind === 'add' ? '' : line.oldNo;
          const newNo = line.kind === 'del' ? '' : line.newNo;
          let sign = '';
          let srLabel = '';
          if (line.kind === 'add') {
            sign = '+';
            srLabel = t('diff.adicionada');
          } else if (line.kind === 'del') {
            sign = '−';
            srLabel = t('diff.removida');
          }
          return (
            <div
              className={cn('diff__line', line.kind !== 'ctx' && `diff__line--${line.kind}`)}
              key={`${line.kind}-${String(oldNo)}-${String(newNo)}-${String(index)}`}
            >
              <span className="diff__num">{oldNo}</span>
              <span className="diff__num">{newNo}</span>
              <span className="diff__sign" aria-hidden="true">
                {sign}
              </span>
              <span className="diff__code">
                {srLabel ? <span className="sr-only">{srLabel}</span> : null}
                {line.text}
              </span>
            </div>
          );
        })}
      </div>
    </figure>
  );
}
