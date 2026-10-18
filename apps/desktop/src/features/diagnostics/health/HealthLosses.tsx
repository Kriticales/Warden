/**
 * "O que tirou pontos" (`healthLosses()` do design system; SPEC T14): uma linha por categoria,
 * na ordem da tabela, com os pontos, o que aconteceu e o link para o problema, o mod ou o teste.
 * A linha dos itens ignorados vem riscada, com "(ignorado por você)", e não tira pontos.
 */
import { Link } from '@tanstack/react-router';
import { Fragment, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type {
  CrashGroupView,
  HealthLine,
  HealthReason,
  PackId,
  ReportFinding,
} from '../../../lib/ipc/bindings';
import { formatDuration, formatMemory } from '../../test/text';
import { crashTitle } from '../crashes/text';
import { findingTitle } from '../text';

/** O que a tela já leu e as linhas citam. */
export interface LossesContext {
  packId: PackId;
  /** Os achados do relatório, pelo id. */
  findings: ReadonlyMap<string, ReportFinding>;
  /** Os grupos de Travamentos, pela assinatura. */
  crashes: ReadonlyMap<string, CrashGroupView>;
  /** Quantas sessões contam em "Travamentos recentes". */
  recentSessions: number;
}

type FindingReason = Extract<HealthReason, { kind: 'finding' }>;

export function HealthLosses({ lines, context }: { lines: HealthLine[]; context: LossesContext }) {
  const { t } = useTranslation('saude');
  return (
    <ul className="losses">
      {lines.map((line) => {
        const capped = !line.ignored && line.rawPoints > line.points;
        const points = line.ignored ? line.rawPoints : line.points;
        return (
          <li
            key={`${line.category}:${String(line.ignored)}`}
            className={line.ignored ? 'is-ignored' : undefined}
          >
            <span className="losses__pts" aria-label={t('pontosRotulo', { pontos: points })}>
              {t('pontos', { pontos: points })}
            </span>
            <span className="losses__txt">
              <b>{t(`categoria.${line.category}`)}</b> <LineText line={line} context={context} />
              {capped ? <span className="t-3"> {t('limite')}</span> : null}
              {line.ignored ? <span className="t-3"> {t('ignorado')}</span> : null}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

function LineText({ line, context }: { line: HealthLine; context: LossesContext }) {
  const { t } = useTranslation('saude');
  const findings = line.reasons.filter(
    (reason): reason is FindingReason => reason.kind === 'finding',
  );
  switch (line.category) {
    case 'problems': {
      const errors = findings.filter((reason) => reason.severity === 'error').length;
      const warnings = findings.length - errors;
      const parts: string[] = [];
      if (errors > 0) parts.push(t('erros', { count: errors }));
      if (warnings > 0) parts.push(t('avisos', { count: warnings }));
      return (
        <>
          {joinParts(parts, t)}
          {': '}
          <FindingLinks reasons={findings} context={context} />.
        </>
      );
    }
    case 'mixins': {
      const high = findings.filter((reason) => reason.risk === 'high').length;
      const medium = findings.length - high;
      const parts: string[] = [];
      if (high > 0) parts.push(t('riscoAlto', { count: high }));
      if (medium > 0) parts.push(t('riscoMedio', { count: medium }));
      return (
        <>
          {joinParts(parts, t)}
          {': '}
          <FindingLinks reasons={findings} context={context} />.
        </>
      );
    }
    case 'download':
      return (
        <>
          {t('download', { count: findings.length })}{' '}
          <Joined
            items={findings.map((reason) => (
              <ItemLink
                key={reason.findingId}
                packId={context.packId}
                path={reason.item}
                label={t('nomeEntreAspas', { nome: itemName(reason, context) ?? t('achado') })}
              />
            ))}
          />
        </>
      );
    case 'maintenance':
      return (
        <>
          <Joined
            items={line.reasons.map((reason) => (
              <MaintenanceEntry key={reasonKey(reason)} reason={reason} context={context} />
            ))}
          />
          .
        </>
      );
    case 'lastTest':
    case 'crashes':
    case 'performance':
      return <SessionReasons line={line} context={context} />;
  }
}

/** "2 erros e 1 aviso". */
function joinParts(parts: string[], t: ReturnType<typeof useTranslation<'saude'>>['t']): string {
  const [first, second] = parts;
  if (first === undefined) return '';
  return second === undefined ? first : t('e', { a: first, b: second });
}

/** Itens separados por "; ". */
function Joined({ items }: { items: ReactNode[] }) {
  return (
    <>
      {items.map((item, index) => (
        // A ordem dos motivos é fixa: o índice não muda entre leituras.
        <Fragment key={index}>
          {index > 0 ? '; ' : null}
          {item}
        </Fragment>
      ))}
    </>
  );
}

function reasonKey(reason: HealthReason): string {
  switch (reason.kind) {
    case 'finding':
      return reason.findingId;
    case 'noNewVersion':
      return `nova:${reason.item}`;
    case 'crash':
      return `crash:${reason.signature}`;
    case 'lastTest':
    case 'slowLoad':
    case 'lowMemory':
      return reason.kind;
  }
}

/** O nome do item do achado: o do relatório ou, sem ele, o arquivo (`mods/clumps.pw.toml` → `clumps`). */
function itemName(reason: FindingReason, context: LossesContext): string | null {
  const entry = context.findings.get(reason.findingId);
  const item = entry?.items.find((candidate) => candidate.path === reason.item) ?? entry?.items[0];
  if (item) return item.name;
  const file = reason.item?.split('/').pop();
  return file ? file.replace(/\.pw\.toml$/, '') : null;
}

/** Os títulos dos achados, cada um com o link para o item (quando ele está no pack). */
function FindingLinks({ reasons, context }: { reasons: FindingReason[]; context: LossesContext }) {
  const { t } = useTranslation('saude');
  return (
    <Joined
      items={reasons.map((reason) => {
        const entry = context.findings.get(reason.findingId);
        return (
          <ItemLink
            key={reason.findingId}
            packId={context.packId}
            path={reason.item}
            label={entry ? findingTitle(entry) : t('achado')}
          />
        );
      })}
    />
  );
}

/** Link para o item na lista de Mods (texto simples quando o item não está no pack). */
function ItemLink({ packId, path, label }: { packId: PackId; path: string | null; label: string }) {
  if (path === null) return <>{label}</>;
  return (
    <Link to="/packs/$packId/mods" params={{ packId }} search={{ item: path }} className="link">
      {label}
    </Link>
  );
}

/** Link para o resultado de um teste. */
function SessionLink({
  packId,
  session,
  children,
}: {
  packId: PackId;
  session: string;
  children: ReactNode;
}) {
  return (
    <Link
      to="/packs/$packId/teste"
      params={{ packId }}
      search={{ sessao: session }}
      className="link"
    >
      {children}
    </Link>
  );
}

function MaintenanceEntry({ reason, context }: { reason: HealthReason; context: LossesContext }) {
  const { t } = useTranslation('saude');
  if (reason.kind === 'noNewVersion') {
    return (
      <ItemLink
        packId={context.packId}
        path={reason.item}
        label={t('semVersaoNova', { nome: reason.name, meses: reason.months })}
      />
    );
  }
  if (reason.kind !== 'finding') return null;
  const nome = itemName(reason, context) ?? t('achado');
  return (
    <ItemLink
      packId={context.packId}
      path={reason.item}
      label={reason.rule === 'I_PRERELEASE' ? t('alfaOuBeta', { nome }) : t('arquivado', { nome })}
    />
  );
}

/** "Último teste", "Travamentos recentes" e "Desempenho": o que aconteceu e o teste. */
function SessionReasons({ line, context }: { line: HealthLine; context: LossesContext }) {
  const { t } = useTranslation('saude');
  const { packId } = context;
  if (line.category === 'crashes') {
    const crashes = line.reasons.filter(
      (reason): reason is Extract<HealthReason, { kind: 'crash' }> => reason.kind === 'crash',
    );
    return (
      <>
        {t('causas', { count: crashes.length, sessoes: context.recentSessions })}{' '}
        <Joined
          items={crashes.map((reason) => {
            const group = context.crashes.get(reason.signature);
            return (
              <SessionLink key={reason.signature} packId={packId} session={reason.session}>
                {group ? crashTitle(group) : t('travamento')}
              </SessionLink>
            );
          })}
        />
      </>
    );
  }
  return (
    <Joined
      items={line.reasons.map((reason) => {
        switch (reason.kind) {
          case 'lastTest':
            return (
              <Fragment key={reason.kind}>
                {t(`ultimoTeste.${reason.state}`)}
                {reason.session !== null ? (
                  <>
                    {' '}
                    <SessionLink packId={packId} session={reason.session}>
                      {t('verTeste')}
                    </SessionLink>
                  </>
                ) : null}
              </Fragment>
            );
          case 'slowLoad':
            return (
              <Fragment key={reason.kind}>
                {t('lento', {
                  agora: formatDuration(reason.loadedMs),
                  antes: formatDuration(reason.goodMs),
                  mais: Math.round((reason.loadedMs / reason.goodMs - 1) * 100),
                })}{' '}
                <SessionLink packId={packId} session={reason.session}>
                  {t('verTeste')}
                </SessionLink>
              </Fragment>
            );
          case 'lowMemory':
            return (
              <Fragment key={reason.kind}>
                {t('memoriaAbaixo', {
                  memoria: formatMemory(reason.memoryMb),
                  sugestao: formatMemory(reason.suggestedMb),
                })}{' '}
                <SessionLink packId={packId} session={reason.session}>
                  {t('verTeste')}
                </SessionLink>
              </Fragment>
            );
          case 'finding':
          case 'noNewVersion':
          case 'crash':
            return null;
        }
      })}
    />
  );
}
