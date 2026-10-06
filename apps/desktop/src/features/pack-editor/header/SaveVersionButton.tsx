/**
 * "Salvar versão · N alterações" (SPEC T05): N é a quantidade de arquivos alterados desde a
 * última versão salva (sem número quando não há nenhuma).
 *
 * O diálogo Salvar versão chega com a V-02, que troca esta linha em `header/slots.ts`. Até lá o
 * botão mostra o contador e fica indisponível, com o motivo na dica (`aria-disabled`, para a
 * dica aparecer no foco e no mouse).
 */
import { Save } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import type { HeaderSlotProps } from './slots';

export function SaveVersionButton({ pack }: HeaderSlotProps) {
  const { t } = useTranslation('editor');
  const count = pack.unsavedFiles;
  const reason = count > 0 ? t('cabecalho.salvarIndisponivel') : t('cabecalho.nadaParaSalvar');
  return (
    <Tooltip content={reason}>
      <Button
        icon={Save}
        aria-disabled="true"
        aria-label={
          count > 0 ? t('cabecalho.salvarVersaoRotulo', { count }) : t('cabecalho.salvarVersao')
        }
        count={
          count > 0 ? (
            <>
              {count}
              <span className="btn__word">&nbsp;{t('cabecalho.alteracoesPalavra', { count })}</span>
            </>
          ) : undefined
        }
        onClick={(event) => {
          event.preventDefault();
        }}
      >
        {t('cabecalho.salvarVersao')}
      </Button>
    </Tooltip>
  );
}
