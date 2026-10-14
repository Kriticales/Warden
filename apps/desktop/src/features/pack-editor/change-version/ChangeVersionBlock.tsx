/**
 * Bloco "Trocar de versão…" dos detalhes do item (SPEC T07; protótipo `detailDrawer`, em
 * **Mais opções**): só para itens do Modrinth e da CurseForge (arquivo local e link direto não
 * têm versões para escolher). Abre o [`ChangeVersionDialog`].
 */
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import type { DetailBlockProps } from '../details/blocks';
import { ChangeVersionDialog } from './ChangeVersionDialog';

export function ChangeVersionBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('trocarVersao');
  const [open, setOpen] = useState(false);
  const opener = useRef<HTMLButtonElement>(null);
  const changeable =
    item.state === 'ok' && (item.source === 'modrinth' || item.source === 'curseforge');
  if (!changeable) return null;
  return (
    <div className="stack-1">
      <div className="btn-row">
        <Button
          ref={opener}
          size="sm"
          onClick={() => {
            setOpen(true);
          }}
        >
          {t('bloco.botao')}
        </Button>
      </div>
      <div className="field__hint">{t('bloco.dica')}</div>
      <ChangeVersionDialog
        packId={packId}
        item={item}
        open={open}
        onOpenChange={setOpen}
        returnFocusTo={() => opener.current}
      />
    </div>
  );
}
