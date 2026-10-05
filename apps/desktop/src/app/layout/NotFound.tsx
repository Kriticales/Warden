/**
 * Rota que não existe (link antigo ou erro de digitação no código): a página diz o que houve e
 * leva de volta ao início, em vez do texto padrão do roteador (em inglês).
 */
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../components/common/EmptyState';
import { buttonVariants } from '../../components/ui/button';
import { AppPage } from './AppPage';
import { PageTitle } from './PageHead';

export function NotFound() {
  const { t } = useTranslation('navegacao');
  return (
    <AppPage>
      <div className="stack">
        <PageTitle className="pagehead__title">{t('naoEncontrada.titulo')}</PageTitle>
        <EmptyState
          glyph="search"
          title={t('naoEncontrada.subtitulo')}
          text={t('naoEncontrada.texto')}
          actions={
            <Link to="/" className={buttonVariants({ variant: 'primary' })}>
              {t('naoEncontrada.voltar')}
            </Link>
          }
        />
      </div>
    </AppPage>
  );
}
