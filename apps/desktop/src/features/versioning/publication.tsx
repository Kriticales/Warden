/**
 * Pontos de extensão da publicação para os jogadores (SPEC T17 e T18), que chega com a V-03.
 * **Registro acréscimo-apenas** (ROADMAP §1): a V-03 troca as duas peças abaixo.
 *
 * - `panel`: a área "Publicação para os jogadores" do Histórico (repositório, visibilidade,
 *   última versão publicada, link do pack com Copiar link e Como os jogadores instalam).
 * - `publishButton`: o botão "Publicar versão X" de uma versão final (no Histórico e no aviso
 *   depois de salvar uma versão final).
 *
 * Até lá, a área só explica como os jogadores recebem o pack e o botão fica indisponível, com o
 * motivo na dica. Nada é enviado ao GitHub.
 */
import { Globe } from 'lucide-react';
import type { ComponentType } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../components/ui/button';
import { Tooltip } from '../../components/ui/tooltip';
import type { PackId, PackRow, SavedVersion } from '../../lib/ipc/bindings';

export interface PublicationPanelProps {
  packId: PackId;
  pack: PackRow;
  /** As versões salvas, da mais nova para a mais antiga. */
  versions: readonly SavedVersion[];
}

export interface PublishButtonProps {
  packId: PackId;
  /** A versão final que o botão publica. */
  version: string;
  size?: 'sm' | 'md';
}

export interface PublicationSlots {
  panel: ComponentType<PublicationPanelProps>;
  publishButton: ComponentType<PublishButtonProps>;
}

function DefaultPublicationPanel() {
  const { t } = useTranslation('versoes');
  return (
    <section className="panel" aria-labelledby="historico-publicacao">
      <h2 className="panel__title" id="historico-publicacao">
        {t('publicacao.titulo')}
      </h2>
      <p className="t-sm t-2 mt-2">{t('publicacao.antes')}</p>
    </section>
  );
}

function DefaultPublishButton({ version, size = 'sm' }: PublishButtonProps) {
  const { t } = useTranslation('versoes');
  return (
    <Tooltip content={t('publicacao.indisponivel')}>
      <Button
        variant="primary"
        size={size}
        icon={Globe}
        aria-disabled="true"
        onClick={(event) => {
          event.preventDefault();
        }}
      >
        {t('publicacao.publicarVersao', { version })}
      </Button>
    </Tooltip>
  );
}

/** Registro acréscimo-apenas: a V-03 troca as duas linhas. */
export const publicationSlots: PublicationSlots = {
  panel: DefaultPublicationPanel,
  publishButton: DefaultPublishButton,
};
