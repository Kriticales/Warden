/**
 * Conferência do arquivo entre as fontes na pré-visualização (SPEC T08): com o projeto nas
 * duas, o SHA-1 do arquivo da versão escolhida é comparado com o das versões da outra fonte; se
 * nenhuma tiver o mesmo, a tela avisa. Não bloqueia nada: é só informação para escolher a fonte.
 */
import { useTranslation } from 'react-i18next';

import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import type { PackId, SourceId, SourceRef } from '../../../lib/ipc/bindings';
import { useProjectVersions } from '../common/api';
import { compareSha1 } from './sha1';

export interface SourceCheckProps {
  packId: PackId;
  /** As duas referências do projeto. */
  sources: readonly SourceRef[];
  /** A fonte escolhida no seletor. */
  source: SourceId;
  /** SHA-1 do arquivo da versão escolhida (`null`: ainda não escolhida ou sem SHA-1). */
  chosenSha1: string | null;
}

export function SourceCheck({ packId, sources, source, chosenSha1 }: SourceCheckProps) {
  const other = sources.find((item) => item.source !== source);
  const versions = useProjectVersions(packId, other?.source ?? source, other?.projectId ?? '');
  const { t } = useTranslation('adicionarCurseforge');
  const { t: tAdd } = useTranslation('adicionar');
  if (!other || chosenSha1 === null) return null;
  if (versions.isPending) return <LoadingState inline label={t('conferencia.lendo')} />;
  // Sem conseguir ler a outra fonte não há o que afirmar: a tela já mostra os avisos de fonte.
  if (versions.isError) return null;
  const match = compareSha1(chosenSha1, versions.data.versions);
  if (match === 'unknown') return null;
  if (match === 'same') {
    return <Alert kind="ok" compact title={t('conferencia.igual')} role="status" />;
  }
  return (
    <Alert kind="warn" compact title={t('conferencia.diferente')} role="status">
      <p>{t('conferencia.diferenteTexto', { fonte: tAdd(`fonte.${other.source}`) })}</p>
    </Alert>
  );
}
