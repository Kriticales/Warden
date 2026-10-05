/**
 * Vídeo incorporado em conteúdo remoto: nada de `iframe` (ARCHITECTURE §18 e §20). Mostra a
 * miniatura (YouTube) e "Abrir no navegador".
 */
import { ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { openExternal } from '../../lib/external';
import { hostOf, videoThumbnail } from '../../lib/sanitize';
import { Button } from '../ui/button';

export function VideoEmbed({ src }: { src: string }) {
  const { t } = useTranslation();
  const thumbnail = videoThumbnail(src);
  const host = hostOf(src);
  return (
    <div className="embed">
      {thumbnail ? (
        <img
          className="embed__thumb"
          src={thumbnail}
          alt={t('conteudo.miniatura')}
          loading="lazy"
        />
      ) : null}
      <span className="embed__host">
        {host ? t('conteudo.videoDe', { host }) : t('conteudo.video')}
      </span>
      <Button
        size="sm"
        iconEnd={ExternalLink}
        onClick={() => {
          void openExternal(src);
        }}
      >
        {t('acoes.abrirNoNavegador')}
      </Button>
    </div>
  );
}
