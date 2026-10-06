/**
 * "▶ Testar ▾" provisório do cabeçalho (SPEC T05 e T13). O teste do pack chega com a L-04, que
 * troca esta linha em `header/slots.ts` pelo botão real (estados "Testando… ver progresso" e
 * "● Jogo aberto: ver teste") e pelo menu ▾ completo, em grupos.
 *
 * Até lá o botão principal fica indisponível, com o motivo na dica, e o ▾ já leva aos Ajustes
 * do teste neste computador (SPEC T11), que a L-04 mantém no grupo "Perfil do teste".
 */
import * as MenuPrimitive from '@radix-ui/react-dropdown-menu';
import { ChevronDown, Play, Settings } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { buttonVariants } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Menu, MenuContent, MenuItem } from '../../../components/ui/menu';
import { Tooltip } from '../../../components/ui/tooltip';
import { cn } from '../../../lib/cn';
import { TestSettingsDialog } from '../test-settings/TestSettingsDialog';
import type { HeaderSlotProps } from './slots';

export function TestButton({ packId, pack }: HeaderSlotProps) {
  const { t } = useTranslation('editor');
  const [settingsOpen, setSettingsOpen] = useState(false);
  return (
    <>
      <div className="testbtn testbtn--disabled">
        <span className="testbtn__antenna" aria-hidden="true">
          <i />
          <i />
          <i />
          <i />
        </span>
        <Tooltip content={t('cabecalho.testarIndisponivel')}>
          <button
            type="button"
            className={cn(buttonVariants({ variant: 'primary' }), 'testbtn__main')}
            aria-disabled="true"
            onClick={(event) => {
              event.preventDefault();
            }}
          >
            <Icon icon={Play} className="icon--fill" />
            <span>{t('cabecalho.testar')}</span>
          </button>
        </Tooltip>
        <Menu>
          <Tooltip content={t('cabecalho.maisOpcoesTeste')}>
            <MenuPrimitive.Trigger
              className={cn(buttonVariants({ variant: 'primary' }), 'testbtn__more')}
            >
              <Icon icon={ChevronDown} />
              <span className="sr-only">{t('cabecalho.maisOpcoesTeste')}</span>
            </MenuPrimitive.Trigger>
          </Tooltip>
          <MenuContent>
            <MenuItem
              icon={Settings}
              description={t('cabecalho.ajustesDoTesteDesc')}
              onSelect={() => {
                setSettingsOpen(true);
              }}
            >
              {t('cabecalho.ajustesDoTeste')}
            </MenuItem>
          </MenuContent>
        </Menu>
      </div>
      <TestSettingsDialog
        packId={packId}
        pack={pack}
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </>
  );
}
