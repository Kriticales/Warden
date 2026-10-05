/**
 * "Pular para o conteúdo" (HANDOFF §7): primeiro item do Tab em toda tela; move o foco para o
 * `<main>` por script (as rotas não usam o hash).
 */
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { focusMain, MAIN_ID } from './focus';

export function SkipLink() {
  const { t } = useTranslation('navegacao');
  return (
    <a
      className="skip-link"
      href={`#${MAIN_ID}`}
      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault();
        focusMain();
      }}
    >
      {t('pularParaConteudo')}
    </a>
  );
}
