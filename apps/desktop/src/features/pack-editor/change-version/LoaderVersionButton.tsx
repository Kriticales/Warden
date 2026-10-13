/**
 * "Trocar versão do loader…" no diálogo Informações do pack (SPEC T11; P1-15). Só aparece com
 * um loader que o catálogo conhece (Forge, NeoForge ou Fabric); pack vanilla não tem.
 */
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import type { PackId, PackMeta } from '../../../lib/ipc/bindings';
import { LoaderVersionDialog, isCatalogLoader } from './LoaderVersionDialog';

export function LoaderVersionButton({ packId, meta }: { packId: PackId; meta: PackMeta }) {
  const { t } = useTranslation('trocarVersao');
  const [open, setOpen] = useState(false);
  const opener = useRef<HTMLButtonElement>(null);
  const loader = meta.loader;
  if (!isCatalogLoader(loader) || !meta.minecraft || !meta.loaderVersion) return null;
  return (
    <>
      <Button
        ref={opener}
        size="sm"
        onClick={() => {
          setOpen(true);
        }}
      >
        {t('loader.botao')}
      </Button>
      <LoaderVersionDialog
        packId={packId}
        loader={loader}
        minecraft={meta.minecraft}
        current={meta.loaderVersion}
        open={open}
        onOpenChange={setOpen}
        returnFocusTo={() => opener.current}
      />
    </>
  );
}
