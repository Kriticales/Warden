/**
 * Um achado como linha do painel "Achados antes do teste" (esboço aprovado do bug-01,
 * "Separadas"; SPEC T14 "Cada achado"): o ícone e a palavra da gravidade, o título em linguagem
 * simples, o que fazer, os itens com link para Mods, a evidência exata em mono e as ações: as
 * correções de sempre, "Ignorar neste pack" e **✦ Perguntar ao Sculk** (conversa nova com o
 * achado como assunto). O cartão completo (`FindingCard`) continua no resultado do teste.
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
import { AskSculkLink } from './AskSculk';

const ICONS: Record<Severity, LucideIcon> = {
  error: CircleX,
  warning: TriangleAlert,
  info: Info,
};

export interface FindingRowProps {
  packId: PackId;
  entry: ReportFinding;
  onFix?: (entry: ReportFinding, fix: SuggestedFix) => void;
  busyFix?: SuggestedFix | null;
  /** "Ignorar neste pack" / "Voltar a mostrar" (só nos achados do pré-teste). */
  onIgnore?: (entry: ReportFinding, ignored: boolean) => void;
  ignoring?: boolean;
  /** Sem "✦ Perguntar ao Sculk" (padrão: com). */
  noSculk?: boolean;
  /** Nível do título (segue o painel: 4 dentro dos grupos de gravidade). */
  headingLevel?: 3 | 4;
  /** A palavra da gravidade à vista (fora dos grupos de gravidade, que já a dizem). */
  showSeverity?: boolean;
}

export function FindingRow({
  packId,
  entry,
  onFix,
  busyFix = null,
  onIgnore,
  ignoring = false,
  noSculk = false,
  headingLevel = 4,
  showSeverity = false,
}: FindingRowProps) {
  const titleId = useId();
  const { t } = useTranslation('diagnostico');
  const kind = severityKind(entry.finding.severity);
  const Heading = `h${String(headingLevel)}` as 'h3' | 'h4';
  const title = findingTitle(entry);
  const text = findingText(entry);
  const fixes = entry.finding.fixes.filter((fix) => fixAvailable(entry, fix));
  const linked = entry.items.flatMap((item) =>
    item.path === null ? [] : [{ path: item.path, name: item.name }],
  );

  return (
    <li>
      <article
        className={`rowlist__row rowlist__row--${kind}${entry.ignored ? ' rowlist__row--ignored' : ''}`}
        aria-labelledby={titleId}
      >
        <Icon icon={ICONS[entry.finding.severity]} size="md" />
        <div className="rowlist__head">
          {showSeverity ? (
            <span className={`badge badge--${kind}`}>
              {t(`gravidade.${entry.finding.severity}`)}
            </span>
          ) : (
            <span className="sr-only">{t(`gravidade.${entry.finding.severity}`)}</span>
          )}
          <Heading className="rowlist__title" id={titleId}>
            {title}
          </Heading>
          {entry.ignored ? <span className="tag tag--plain">{t('achado.ignorado')}</span> : null}
        </div>
        <p className="rowlist__text">{text}</p>
        {linked.length > 0 ? (
          <p className="rowlist__text">
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
        <ul className="rowlist__ev diag-row__proofs" aria-label={t('achado.comoSabemos')}>
          {entry.finding.evidence.map((evidence) => {
            const view = evidenceView(entry, evidence);
            return (
              <li key={JSON.stringify(evidence)}>
                <span>{view.label}</span>
                {view.excerpt ? <span className="diag-row__excerpt">{view.excerpt}</span> : null}
              </li>
            );
          })}
        </ul>
        <div className="rowlist__actions">
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
          {noSculk ? null : (
            <AskSculkLink
              packId={packId}
              subject={{
                sintoma: 'other',
                assunto: t('sculk.assunto', { titulo: title, texto: text }),
              }}
              label={t('sculk.perguntarRotulo', { titulo: title })}
            >
              {t('sculk.perguntar')}
            </AskSculkLink>
          )}
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
      </article>
    </li>
  );
}
