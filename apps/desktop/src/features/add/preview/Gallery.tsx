/**
 * Galeria da pré-visualização (SPEC T08, P1; protótipo `galleryStrip`): miniaturas numa tira;
 * clicar numa abre a imagem inteira num diálogo sobre a página, com anterior e próxima. As
 * imagens da CurseForge chegam pelo protocolo `warden-img://` (só memória).
 */
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { imageSrc } from '../../../lib/ipc/image-src';
import type { GalleryItem, PackId, SourceId } from '../../../lib/ipc/bindings';
import { useProjectGallery } from './api';

export interface GalleryProps {
  packId: PackId;
  source: SourceId;
  projectId: string;
}

export function Gallery({ packId, source, projectId }: GalleryProps) {
  const { t } = useTranslation('descoberta');
  const gallery = useProjectGallery(packId, source, projectId);
  const [open, setOpen] = useState<number | null>(null);
  if (gallery.isPending) return <LoadingState inline label={t('galeria.carregando')} />;
  if (gallery.isError) {
    return (
      <ErrorPanel
        compact
        title={t('galeria.erro')}
        error={gallery.error}
        onRetry={() => {
          void gallery.refetch();
        }}
      />
    );
  }
  const items = gallery.data;
  if (items.length === 0) return <p className="t-sm t-3">{t('galeria.vazia')}</p>;
  const label = (item: GalleryItem, index: number) =>
    item.title ?? t('galeria.semTitulo', { n: index + 1 });
  const current = open === null ? null : items[open];
  return (
    <>
      <ul className="gallery" aria-label={t('galeria.lista')}>
        {items.map((item, index) => (
          <li key={item.url}>
            <button
              type="button"
              className="gallery__item"
              aria-label={t('galeria.abrir', { name: label(item, index) })}
              onClick={() => {
                setOpen(index);
              }}
            >
              <img src={imageSrc(item.thumbUrl) ?? undefined} alt="" loading="lazy" />
            </button>
            <span className="gallery__cap">{label(item, index)}</span>
          </li>
        ))}
      </ul>
      <Dialog
        open={current !== undefined && current !== null}
        onOpenChange={(next) => {
          if (!next) setOpen(null);
        }}
      >
        {current && open !== null ? (
          <DialogContent
            size="lg"
            title={label(current, open)}
            description={current.description ?? undefined}
            footerSplit
            footer={
              <>
                <Button
                  icon={ChevronLeft}
                  disabled={open === 0}
                  onClick={() => {
                    setOpen(open - 1);
                  }}
                >
                  {t('galeria.anterior')}
                </Button>
                <span className="t-xs t-3" aria-live="polite">
                  {t('galeria.visualizador', { n: open + 1, total: items.length })}
                </span>
                <Button
                  iconEnd={ChevronRight}
                  disabled={open === items.length - 1}
                  onClick={() => {
                    setOpen(open + 1);
                  }}
                >
                  {t('galeria.proxima')}
                </Button>
              </>
            }
          >
            <BigImage key={current.url} item={current} alt={label(current, open)} />
          </DialogContent>
        ) : null}
      </Dialog>
    </>
  );
}

function BigImage({ item, alt }: { item: GalleryItem; alt: string }) {
  const { t } = useTranslation('descoberta');
  const [failed, setFailed] = useState(false);
  if (failed) return <p className="t-sm t-3">{t('galeria.naoCarregou')}</p>;
  return (
    <img
      className="gallery__big"
      src={imageSrc(item.url) ?? undefined}
      alt={alt}
      onError={() => {
        setFailed(true);
      }}
    />
  );
}
