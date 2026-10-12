/**
 * "Salvar versão · N alterações" no cabeçalho do pack (SPEC T05 e T16): N é a quantidade de
 * arquivos alterados desde a última versão salva (sem número quando não há nenhuma). Abre o
 * diálogo Salvar versão. Sem alterações, ou com o pack somente leitura, o botão fica
 * indisponível com o motivo na dica (`aria-disabled`, para a dica aparecer no foco e no mouse).
 */
import { Save } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import type { PackRow } from '../../../lib/ipc/bindings';
import { SaveVersionFlow } from './SaveVersionDialog';

export function SaveVersionButton({ pack }: { pack: PackRow }) {
  const { t } = useTranslation('versoes');
  const [open, setOpen] = useState(false);
  const count = pack.unsavedFiles;
  const blocked =
    pack.readOnlyReason !== null
      ? t('cabecalho.somenteLeitura', { motivo: pack.readOnlyReason })
      : count === 0
        ? t('cabecalho.nada')
        : null;

  const button = (
    <Button
      icon={Save}
      aria-disabled={blocked !== null ? 'true' : undefined}
      aria-label={count > 0 ? t('cabecalho.rotulo', { count }) : t('cabecalho.salvarVersao')}
      count={
        count > 0 ? (
          <>
            {count}
            <span className="btn__word">&nbsp;{t('cabecalho.alteracoesPalavra', { count })}</span>
          </>
        ) : undefined
      }
      onClick={(event) => {
        if (blocked !== null) {
          event.preventDefault();
          return;
        }
        setOpen(true);
      }}
    >
      {t('cabecalho.salvarVersao')}
    </Button>
  );

  return (
    <>
      {blocked !== null ? <Tooltip content={blocked}>{button}</Tooltip> : button}
      <SaveVersionFlow packId={pack.id} open={open} onOpenChange={setOpen} />
    </>
  );
}
