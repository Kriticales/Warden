/**
 * Ícone remoto de um projeto (busca, cartões, pré-visualização, diálogo Adicionar itens); sem
 * ícone, ou se ele não carregar, o bloco em pixel do nome.
 */
import { useState } from 'react';

import { NameTile, type TileSize } from '../../../components/common/PixelArt';
import { cn } from '../../../lib/cn';
import { imageSrc } from '../../../lib/ipc/image-src';

export function ProjectIcon({
  url,
  name,
  className,
  size,
}: {
  url: string | null;
  name: string;
  className: string;
  size: TileSize;
}) {
  const [failed, setFailed] = useState(false);
  if (!url || failed) return <NameTile seed={name} size={size} />;
  return (
    <img
      className={cn('tile', `tile--${size}`, className)}
      src={imageSrc(url) ?? undefined}
      alt=""
      loading="lazy"
      onError={() => {
        setFailed(true);
      }}
    />
  );
}
