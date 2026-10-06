/**
 * Cabeçalho dos detalhes (SPEC T07; protótipo `detailDrawer`): ícone, nome, autores, fonte e
 * "Abrir página" (no navegador do sistema, só `https:`).
 */
import { ExternalLink } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../../components/common/PixelArt';
import { Button } from '../../../../components/ui/button';
import { openExternal } from '../../../../lib/external';
import { SourceTag } from '../../mods/marks';
import type { DetailBlockProps } from '../blocks';

export function HeadBlock({ item, details }: DetailBlockProps) {
  const { t } = useTranslation('editor');
  const title = details?.title.trim() ? details.title : item.name;
  const icon = details?.iconUrl ?? item.iconUrl;
  const authors = details?.authors ?? [];
  const page = details?.pageUrl;
  // Sem internet (ou imagem quebrada), o ícone vira o desenho gerado do nome.
  const [broken, setBroken] = useState<string | null>(null);
  return (
    <div className="itemhead">
      {icon && broken !== icon ? (
        <img
          className="tile tile--xl itemhead__icon"
          src={icon}
          alt=""
          loading="lazy"
          onError={() => {
            setBroken(icon);
          }}
        />
      ) : (
        <NameTile seed={item.name} size="xl" />
      )}
      <div className="itemhead__titles">
        <div className="t-display-lg itemhead__name">{title}</div>
        <div className="t-xs t-3 row row--wrap itemhead__meta">
          {authors.length > 0 ? (
            <span>{t('detalhes.por', { autores: authors.join(', ') })}</span>
          ) : null}
          {item.state === 'invalid' ? null : <SourceTag source={item.source} />}
          {page ? (
            <Button
              variant="link"
              size="sm"
              iconEnd={ExternalLink}
              onClick={() => {
                void openExternal(page);
              }}
            >
              {t('detalhes.abrirPagina')}
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
