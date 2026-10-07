/**
 * O passo a passo para o jogador (SPEC T18/T19): arrastar o arquivo para o Prism, confirmar os
 * mods de fora do Modrinth, memória e a diferença para o caminho com atualização automática.
 * Reaproveitado pelo resultado do Publicar versão (V-03).
 */
import { useTranslation } from 'react-i18next';

export function PrismSteps({ memory }: { memory?: string | undefined }) {
  const { t } = useTranslation('instanciaPrism');
  return (
    <section aria-labelledby="prism-passos">
      <h3 className="t-sm" id="prism-passos">
        {t('passos.titulo')}
      </h3>
      <ol className="mt-2 list-decimal pl-5 t-sm" aria-label={t('passos.rotulo')}>
        <li>{t('passos.um')}</li>
        <li>{t('passos.dois')}</li>
        <li>{t('passos.tres', { memoria: memory ?? t('passos.memoriaPadrao') })}</li>
      </ol>
      <p className="mt-2 t-sm t-2">{t('passos.atualizar')}</p>
    </section>
  );
}
