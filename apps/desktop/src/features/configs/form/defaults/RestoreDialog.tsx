/**
 * "Restaurar padrão" no editor (SPEC T12, parte P1; C-06): a diferença **antes** de mudar
 * qualquer coisa, para uma chave ou para o arquivo todo. Confirmar põe o texto restaurado no
 * editor (só os bytes dos valores mudam); gravar é pelo Salvar de sempre, e Descartar ou
 * Ctrl+Z voltam atrás. O foco volta para quem abriu o diálogo (Radix).
 */
import { RotateCcw } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../../components/common/DiffView';
import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { LoadingState } from '../../../../components/common/LoadingState';
import { Button } from '../../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../../components/ui/dialog';
import type { PackId } from '../../../../lib/ipc/bindings';
import { baseName } from '../../model';
import { useRestorePreview } from './api';
import './defaults.css';

/** O que restaurar: uma chave (caminho como o formulário mostra) ou o arquivo todo. */
export type RestoreRequest = { kind: 'key'; key: string } | { kind: 'file' };

export interface RestoreDialogProps {
  packId: PackId;
  path: string;
  /** O texto atual do editor. */
  text: string;
  /** O pedido aberto, ou `null` com o diálogo fechado. */
  request: RestoreRequest | null;
  onClose: () => void;
  /** O texto restaurado vai para o editor. */
  onApply: (text: string) => void;
  /** O editor: quem abriu pode ter sumido (o botão da chave sai quando ela volta ao padrão),
   * então o foco volta para o campo da chave ou para o menu ⋯ dentro dele. */
  scope: () => HTMLElement | null;
}

/** O primeiro elemento focável do campo da chave, ou o gatilho do menu ⋯. */
function focusTarget(scope: HTMLElement | null, request: RestoreRequest | null) {
  if (!scope || !request) return null;
  if (request.kind === 'file') {
    return scope.querySelector<HTMLElement>('[data-defaults-menu] button');
  }
  const row = [...scope.querySelectorAll<HTMLElement>('.cfgform__row[data-key]')].find(
    (candidate) => candidate.dataset.key === request.key,
  );
  return row?.querySelector<HTMLElement>('input, select, textarea, button') ?? null;
}

export function RestoreDialog({
  packId,
  path,
  text,
  request,
  onClose,
  onApply,
  scope,
}: RestoreDialogProps) {
  const { t } = useTranslation('configsPadrao');
  // O pedido continua conhecido quando o diálogo termina de fechar.
  const last = useRef<RestoreRequest | null>(null);
  useEffect(() => {
    if (request) last.current = request;
  }, [request]);
  const keys = request?.kind === 'key' ? [request.key] : null;
  const preview = useRestorePreview(packId, path, text, keys, request !== null);
  const restored = preview.data;
  const title =
    request?.kind === 'key'
      ? t('dialogo.tituloChave', { chave: request.key })
      : t('dialogo.tituloArquivo', { arquivo: baseName(path) });

  return (
    <Dialog
      open={request !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        size="lg"
        title={title}
        description={t('dialogo.sub')}
        returnFocusTo={() => focusTarget(scope(), last.current)}
        footer={
          <>
            <DialogClose asChild>
              <Button variant="ghost">{t('dialogo.cancelar')}</Button>
            </DialogClose>
            <Button
              variant="primary"
              icon={RotateCcw}
              disabled={!restored || restored.keys.length === 0}
              onClick={() => {
                if (restored) onApply(restored.text);
                onClose();
              }}
            >
              {t('dialogo.confirmar')}
            </Button>
          </>
        }
      >
        {preview.isError ? (
          <ErrorPanel compact title={t('dialogo.erro')} error={preview.error} />
        ) : !restored ? (
          <LoadingState label={t('dialogo.calculando')} />
        ) : (
          <>
            <p className="t-sm t-2 cfg-restore__count">
              {t('dialogo.chaves', { count: restored.keys.length })}
            </p>
            <DiffView file={path} before={text} after={restored.text} />
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
