/**
 * Contador da seção Problemas no menu do pack (SPEC T14 "com contador no menu"): os erros e os
 * avisos que valem (sem os ignorados), em cor de perigo quando há erro e de atenção quando só há
 * avisos; nada quando não há problema ou o pack nunca foi verificado.
 */
import { useTranslation } from 'react-i18next';

import type { PackId } from '../../lib/ipc/bindings';
import type { SectionCount } from '../pack-editor/sections';
import { useDiagnosticsReport } from './api';
import { countBySeverity } from './text';

export function useProblemsCount(packId: PackId): SectionCount | null {
  const { t } = useTranslation('diagnostico');
  const report = useDiagnosticsReport(packId);
  if (!report.data) {
    return null;
  }
  const counts = countBySeverity(report.data.findings);
  const value = counts.error + counts.warning;
  if (value === 0) {
    return null;
  }
  return {
    value,
    kind: counts.error > 0 ? 'danger' : 'warn',
    label: t('secao.contagem', { count: value }),
  };
}
