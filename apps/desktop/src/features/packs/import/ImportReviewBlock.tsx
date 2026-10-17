/**
 * "N itens para revisar" na seção Problemas (SPEC T24: o pack importado abre "com o aviso 'N
 * itens para revisar' em Problemas"; registrado em `diagnostics/problems-blocks.ts`). Lista os
 * itens que a importação marcou (lado desconhecido, opcionais, `client-overrides/`, jars que
 * ficaram como arquivo local), cada um com **Revisar**, que abre o item em Mods (ou o arquivo em
 * Configs), e **Já revisei**, que esconde o aviso neste computador. Itens que saíram do pack
 * não contam; pack não importado não mostra nada.
 */
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { ImportPendingItem, PackId } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import type { ProblemsBlockProps } from '../../diagnostics/problems-blocks';
import { useImportReview, useImportReviewDone } from './api';
import './import.css';

/** Mods e jars abrem os detalhes em Mods; o resto (configs de `client-overrides/`), em Configs. */
function isModItem(path: string) {
  return path.endsWith('.pw.toml') || path.endsWith('.jar');
}

function ReviewLink({ packId, item }: { packId: PackId; item: ImportPendingItem }) {
  const { t } = useTranslation('importar');
  const label = t('revisao.revisar', { name: item.name });
  return (
    <Button size="sm" variant="ghost" asChild>
      {isModItem(item.path) ? (
        <Link
          to="/packs/$packId/mods"
          params={{ packId }}
          search={{ item: item.path }}
          aria-label={label}
        >
          {t('revisao.revisarCurto')}
        </Link>
      ) : (
        <Link
          to="/packs/$packId/configs"
          params={{ packId }}
          search={{ arquivo: item.path }}
          aria-label={label}
        >
          {t('revisao.revisarCurto')}
        </Link>
      )}
    </Button>
  );
}

export function ImportReviewBlock({ packId }: ProblemsBlockProps) {
  const { t } = useTranslation('importar');
  const review = useImportReview(packId);
  const done = useImportReviewDone(packId);
  const pending = review.data;
  if (!pending) {
    return null;
  }
  const count = pending.items.length;
  return (
    <Alert
      kind="warn"
      title={t('revisao.titulo', { count })}
      actions={
        <Button
          size="sm"
          loading={done.isPending}
          onClick={() => {
            done.mutate(undefined, {
              onError: (error) => {
                showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
              },
            });
          }}
        >
          {t('revisao.pronto')}
        </Button>
      }
    >
      <div className="alert__text">{t('revisao.texto', { arquivo: pending.file })}</div>
      <ul className="import-review__list">
        {pending.items.map((item) => (
          <li key={item.path} className="import-review__item">
            <div className="import__row-body">
              <span className="t-strong">{item.name}</span>{' '}
              <span className="t-xs t-3">{t(`revisao.motivo.${item.reason}`)}</span>
            </div>
            <ReviewLink packId={packId} item={item} />
          </li>
        ))}
      </ul>
    </Alert>
  );
}
