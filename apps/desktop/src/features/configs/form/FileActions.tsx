/**
 * Ações dos avisos contextuais de um arquivo do pack (SPEC T12, P1; C-04):
 *
 * - `options.txt`: **Não substituir se o jogador já tiver** (grava `preserve = true` no índice);
 * - `config/*-server.toml`: **Copiar para defaultconfigs/**, com confirmação antes de substituir
 *   um arquivo diferente que já esteja lá.
 *
 * As duas gravam no pack e invalidam tudo o que mostra o pack (`form/api.ts`).
 */
import { Copy } from 'lucide-react';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type { CopyOutcome, PackId } from '../../../lib/ipc/bindings';
import { Checkbox } from '../../settings/components/fields';
import { baseName } from '../model';
import { useCopyToDefaultconfigs, usePreserve, useSetPreserve } from './api';

export interface PreserveOptionProps {
  packId: PackId;
  path: string;
}

/** A caixa "Não substituir se o jogador já tiver" do aviso do `options.txt`. */
export function PreserveOption({ packId, path }: PreserveOptionProps) {
  const { t } = useTranslation('configsFormulario');
  const state = usePreserve(packId, path, true);
  const change = useSetPreserve(packId);
  const name = baseName(path);
  if (state.isPending) return null;
  if (state.isError) {
    return (
      <ErrorPanel compact error={state.error} title={t('preservar.erro', { arquivo: name })} />
    );
  }
  const inIndex = state.data.inIndex;
  return (
    <div className="cfg-preserve">
      <Checkbox
        label={t('preservar.rotulo')}
        desc={inIndex ? t('preservar.dica') : t('preservar.foraDoIndice')}
        checked={state.data.preserve}
        disabled={!inIndex || change.isPending}
        onChange={(preserve) => {
          change.mutate(
            { path, preserve },
            {
              onSuccess: () => {
                showToast({
                  kind: 'ok',
                  title: t(preserve ? 'preservar.ligadoToast' : 'preservar.desligadoToast', {
                    arquivo: name,
                  }),
                });
              },
            },
          );
        }}
      />
      {change.isError ? (
        <ErrorPanel compact error={change.error} title={t('preservar.erro', { arquivo: name })} />
      ) : null}
    </div>
  );
}

export interface CopyToDefaultconfigsProps {
  packId: PackId;
  path: string;
  /** Com alterações não salvas, a cópia levaria o arquivo antigo: pede para salvar antes. */
  dirty: boolean;
}

/** O botão "Copiar para defaultconfigs/" do aviso do `*-server.toml`. */
export function CopyToDefaultconfigs({ packId, path, dirty }: CopyToDefaultconfigsProps) {
  const { t } = useTranslation('configsFormulario');
  const copy = useCopyToDefaultconfigs(packId);
  const [confirm, setConfirm] = useState<string | null>(null);
  const name = baseName(path);
  // O botão fica desabilitado enquanto copia e o diálogo abre por estado: o foco volta à mão.
  const trigger = useRef<HTMLButtonElement>(null);
  const refocus = () => {
    requestAnimationFrame(() => {
      trigger.current?.focus();
    });
  };

  const done = (outcome: CopyOutcome) => {
    if (outcome.kind === 'needsOverwrite') {
      setConfirm(outcome.target);
      return;
    }
    setConfirm(null);
    refocus();
    showToast({
      kind: outcome.kind === 'copied' ? 'ok' : 'info',
      title: t(outcome.kind === 'copied' ? 'copiar.copiadoToast' : 'copiar.igualToast', {
        destino: outcome.target,
      }),
    });
  };

  const button = (
    <Button
      ref={trigger}
      size="sm"
      icon={Copy}
      aria-disabled={dirty ? true : undefined}
      loading={copy.isPending && confirm === null}
      onClick={() => {
        if (dirty) return;
        copy.mutate({ path, overwrite: false }, { onSuccess: done });
      }}
    >
      {t('copiar.botao')}
    </Button>
  );

  return (
    <>
      {dirty ? <Tooltip content={t('copiar.salveAntes')}>{button}</Tooltip> : button}
      {copy.isError ? (
        <ErrorPanel compact error={copy.error} title={t('copiar.erro', { arquivo: name })} />
      ) : null}
      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) {
            setConfirm(null);
            refocus();
          }
        }}
        title={t('copiar.substituirTitulo', { destino: confirm ?? '' })}
        description={t('copiar.substituirTexto')}
        confirmLabel={t('copiar.substituirConfirmar')}
        onConfirm={async () => {
          done(await copy.mutateAsync({ path, overwrite: true }));
        }}
      />
    </>
  );
}
