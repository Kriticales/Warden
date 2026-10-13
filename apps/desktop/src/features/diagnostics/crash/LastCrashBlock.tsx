/**
 * "Último travamento" na seção Problemas (ROADMAP D-03; substituído pela lista de Travamentos
 * da D-06): o teste mais novo que travou, com a causa em linguagem simples (ou "Causa não
 * encontrada") e "Ver o que causou", que abre o resultado desse teste.
 */
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { useTestSessions } from '../../test/api';
import { formatWhen } from '../../test/text';
import { useCrashDiagnosis } from '../api';
import type { ProblemsBlockProps } from '../problems-blocks';
import { findingTitle } from '../text';

export function LastCrashBlock({ packId }: ProblemsBlockProps) {
  const { t } = useTranslation('diagnostico');
  const sessions = useTestSessions(packId);
  const crashed = sessions.data?.find((session) => session.outcome === 'crashed') ?? null;
  const diagnosis = useCrashDiagnosis(packId, crashed?.id ?? '', crashed !== null);

  if (!sessions.isSuccess) {
    return null;
  }
  const primary = diagnosis.data?.foundCause ? diagnosis.data.findings[0] : undefined;
  let cause: string;
  if (!crashed) {
    cause = t('ultimo.nenhum');
  } else if (diagnosis.isPending) {
    cause = t('travou.analisando');
  } else if (primary) {
    cause = findingTitle(primary);
  } else {
    cause = t('travou.semCausaTitulo');
  }
  return (
    <section className="panel stack-2" aria-labelledby="diag-last-crash">
      <h2 className="group-title" id="diag-last-crash">
        {t('ultimo.titulo')}
      </h2>
      <div className="row row--between row--wrap row--gap-3">
        <div>
          <p className="t-strong">{cause}</p>
          {crashed ? (
            <p className="t-xs t-3">
              {crashed.packVersion
                ? t('ultimo.quando', {
                    quando: formatWhen(crashed.startedAtMs),
                    versao: crashed.packVersion,
                  })
                : t('ultimo.quandoSemVersao', { quando: formatWhen(crashed.startedAtMs) })}
            </p>
          ) : null}
        </div>
        {crashed ? (
          <Button size="sm" asChild>
            <Link to="/packs/$packId/teste" params={{ packId }} search={{ sessao: crashed.id }}>
              {t('ultimo.ver')}
            </Link>
          </Button>
        ) : null}
      </div>
    </section>
  );
}
