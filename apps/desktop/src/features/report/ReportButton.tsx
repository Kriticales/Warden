/**
 * Botão "Relatar problema" (UI-02), sempre à mão: na barra de título própria (Windows; UI-01)
 * ou, onde a janela usa a barra do sistema (Linux), no rodapé. A dica mostra o atalho.
 */
import { MessageSquareWarning } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../components/ui/icon';
import { Tooltip } from '../../components/ui/tooltip';
import { cn } from '../../lib/cn';
import { openReportDialog } from './store';
import './report.css';

/** Gancho dos testes. */
const TEST_ID = 'report-button';

/** O botão na tela (só um existe: na barra de título ou no rodapé). */
let current: HTMLButtonElement | null = null;

/** O botão "Relatar problema" na tela, para o foco voltar a ele ao fechar o diálogo. */
export function reportButtonElement(): HTMLButtonElement | null {
  return current;
}

export interface ReportButtonProps {
  /** Onde o botão fica: barra de título ou rodapé. */
  place: 'titlebar' | 'statusbar';
}

export function ReportButton({ place }: ReportButtonProps) {
  const { t } = useTranslation('relato');
  return (
    <Tooltip
      content={t('dica', { atalho: t('atalho') })}
      side={place === 'titlebar' ? 'bottom' : 'top'}
    >
      <button
        type="button"
        className={cn(
          place === 'titlebar' ? 'titlebar__report' : 'statusbar__tasks statusbar__report',
        )}
        aria-keyshortcuts="Control+Shift+E"
        data-testid={TEST_ID}
        ref={(element) => {
          current = element;
        }}
        onClick={openReportDialog}
      >
        <Icon icon={MessageSquareWarning} size="sm" />
        <span>{t('botao')}</span>
      </button>
    </Tooltip>
  );
}
