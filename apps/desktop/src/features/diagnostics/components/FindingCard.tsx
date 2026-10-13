/**
 * Um achado (SPEC T14 "Cada achado"; protótipo `W.issue`): a gravidade com ícone e palavra, o
 * título em linguagem simples, a explicação (o que fazer), os itens envolvidos com link para a
 * lista de Mods, "Como sabemos" (a evidência exata, recolhida) e os botões de correção. "Ignorar
 * neste pack" aparece quando a tela oferece (Problemas).
 */
import { Link } from '@tanstack/react-router';
import { CircleX, Info, TriangleAlert, type LucideIcon } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { PackId, ReportFinding, Severity, SuggestedFix } from '../../../lib/ipc/bindings';
import {
  evidenceView,
  findingText,
  findingTitle,
  fixAvailable,
  fixLabel,
  severityKind,
} from '../text';

const ICONS: Record<Severity, LucideIcon> = {
  error: CircleX,
  warning: TriangleAlert,
  info: Info,
};

export interface FindingCardProps {
  packId: PackId;
  entry: ReportFinding;
  /** Nível do título (segue a página). */
  headingLevel?: 3 | 4;
  /** Evidência aberta (o achado principal de um travamento). */
  evidenceOpen?: boolean;
  /** Correções mostradas (o diálogo do Testar mostra só a principal). */
  maxFixes?: number;
  onFix?: (entry: ReportFinding, fix: SuggestedFix) => void;
  busyFix?: SuggestedFix | null;
  /** "Ignorar neste pack" / "Voltar a mostrar". */
  onIgnore?: (entry: ReportFinding, ignored: boolean) => void;
  ignoring?: boolean;
}

export function FindingCard({
  packId,
  entry,
  headingLevel = 3,
  evidenceOpen = false,
  maxFixes,
  onFix,
  busyFix = null,
  onIgnore,
  ignoring = false,
}: FindingCardProps) {
  const { t } = useTranslation('diagnostico');
  const titleId = useId();
  const kind = severityKind(entry.finding.severity);
  const Heading = `h${String(headingLevel)}` as 'h3' | 'h4';
  const fixes = entry.finding.fixes
    .filter((fix) => fixAvailable(entry, fix))
    .slice(0, maxFixes ?? Number.POSITIVE_INFINITY);
  const linked = entry.items.flatMap((item) =>
    item.path === null ? [] : [{ path: item.path, name: item.name }],
  );

  return (
    <article
      className={`issue issue--${kind}${entry.ignored ? ' issue--ignored' : ''}`}
      aria-labelledby={titleId}
    >
      <div className="issue__head">
        <span className={`badge badge--${kind}`}>
          <Icon icon={ICONS[entry.finding.severity]} />
          {t(`gravidade.${entry.finding.severity}`)}
        </span>
        <Heading className="issue__title" id={titleId}>
          {findingTitle(entry)}
        </Heading>
        {entry.ignored ? <span className="tag tag--plain">{t('achado.ignorado')}</span> : null}
      </div>
      <p className="issue__text">{findingText(entry)}</p>
      {linked.length > 0 ? (
        <p className="issue__items t-sm">
          <span className="t-3">{t('achado.itens')}: </span>
          {linked.map((item, index) => (
            <span key={item.path}>
              {index > 0 ? ', ' : null}
              <Link
                to="/packs/$packId/mods"
                params={{ packId }}
                search={{ item: item.path }}
                className="link"
                aria-label={t('achado.verItem', { nome: item.name })}
              >
                {item.name}
              </Link>
            </span>
          ))}
        </p>
      ) : null}
      <details className="issue__evidence" open={evidenceOpen}>
        <summary>{t('achado.comoSabemos')}</summary>
        <ul className="issue__proofs">
          {entry.finding.evidence.map((evidence) => {
            const view = evidenceView(entry, evidence);
            return (
              <li key={JSON.stringify(evidence)}>
                <span>{view.label}</span>
                {view.excerpt ? <pre className="code issue__excerpt">{view.excerpt}</pre> : null}
              </li>
            );
          })}
        </ul>
      </details>
      {fixes.length > 0 || onIgnore ? (
        <div className="issue__actions">
          {fixes.map((fix, index) => (
            <Button
              key={JSON.stringify(fix)}
              size="sm"
              variant={index === 0 && !entry.ignored ? 'primary' : 'secondary'}
              loading={busyFix === fix}
              disabled={busyFix !== null && busyFix !== fix}
              onClick={() => {
                onFix?.(entry, fix);
              }}
            >
              {fixLabel(entry, fix)}
            </Button>
          ))}
          {onIgnore ? (
            <Button
              size="sm"
              variant="ghost"
              loading={ignoring}
              onClick={() => {
                onIgnore(entry, !entry.ignored);
              }}
            >
              {entry.ignored ? t('achado.desfazerIgnorar') : t('achado.ignorar')}
            </Button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}
