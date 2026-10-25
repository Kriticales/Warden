/**
 * As ações da seção Configs com as três origens (SPEC T12; ADR-0067), num lugar só para a
 * faixa do editor, o painel Detalhes e o Comparar: **Trazer para o pack**, **Voltar ao do
 * pack**, **Marcar como pessoal** e **Deixar de ser pessoal…** (essas duas confirmam antes).
 */
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { showToast } from '../../../components/ui/toast';
import type { PackId, PlacedConfig } from '../../../lib/ipc/bindings';
import { baseName } from '../model';
import { useBring, useMarkPersonal, useRevert, useUnmarkPersonal } from './api';

export interface PlaceActions {
  /** Trazer o arquivo inteiro (sem `keys`) ou só essas chaves para o pack. */
  bring: (path: string, keys?: string[]) => void;
  /** Voltar o teste ao do pack: o arquivo inteiro ou só essas chaves. */
  revert: (path: string, keys?: string[]) => void;
  /** O que está sendo trazido agora (`caminho` ou `caminho|chave`). */
  busy: string | null;
  markPersonal: (file: PlacedConfig) => void;
  unmarkPersonal: (file: PlacedConfig) => void;
  /** Os diálogos de confirmação (montar uma vez na página). */
  dialogs: ReactNode;
}

function busyKey(path: string, keys?: string[]): string {
  return keys?.length === 1 ? `${path}|${keys[0] ?? ''}` : path;
}

export function usePlaceActions(packId: PackId): PlaceActions {
  const { t } = useTranslation('configs');
  const bringMutation = useBring(packId);
  const revertMutation = useRevert(packId);
  const mark = useMarkPersonal(packId);
  const unmark = useUnmarkPersonal(packId);
  const [busy, setBusy] = useState<string | null>(null);
  const [marking, setMarking] = useState<PlacedConfig | null>(null);
  const [unmarking, setUnmarking] = useState<PlacedConfig | null>(null);

  const bring = (path: string, keys?: string[]) => {
    setBusy(busyKey(path, keys));
    bringMutation.mutate(
      { path, keys: keys ?? null },
      {
        onSuccess: () => {
          showToast({ kind: 'ok', title: t('faixa.trazidoToast', { arquivo: baseName(path) }) });
        },
        onSettled: () => {
          setBusy(null);
        },
      },
    );
  };
  const revert = (path: string, keys?: string[]) => {
    setBusy(busyKey(path, keys));
    revertMutation.mutate(
      { path, keys: keys ?? null },
      {
        onSuccess: () => {
          showToast({ kind: 'ok', title: t('faixa.voltouToast', { arquivo: baseName(path) }) });
        },
        onSettled: () => {
          setBusy(null);
        },
      },
    );
  };

  const dialogs = (
    <>
      <ConfirmDialog
        open={marking !== null}
        onOpenChange={(open) => {
          if (!open) setMarking(null);
        }}
        title={t('pessoal.marcarTitulo', { arquivo: marking ? baseName(marking.path) : '' })}
        description={
          marking?.inPack
            ? `${t('pessoal.marcarTexto')} ${t('pessoal.marcarTextoPack')}`
            : t('pessoal.marcarTexto')
        }
        confirmLabel={t('pessoal.marcarConfirmar')}
        destructive={marking?.inPack ?? false}
        onConfirm={async () => {
          if (!marking) return;
          const file = marking;
          await mark.mutateAsync(file.path);
          setMarking(null);
          showToast({
            kind: 'ok',
            title: t('pessoal.marcadoToast', { arquivo: baseName(file.path) }),
          });
        }}
      />
      <ConfirmDialog
        open={unmarking !== null}
        onOpenChange={(open) => {
          if (!open) setUnmarking(null);
        }}
        title={t('pessoal.desmarcarTitulo', {
          arquivo: unmarking ? baseName(unmarking.path) : '',
        })}
        description={t('pessoal.desmarcarTexto')}
        confirmLabel={t('pessoal.desmarcarConfirmar')}
        destructive={false}
        onConfirm={async () => {
          if (!unmarking) return;
          const file = unmarking;
          await unmark.mutateAsync(file.path);
          setUnmarking(null);
          showToast({
            kind: 'ok',
            title: t('pessoal.desmarcadoToast', { arquivo: baseName(file.path) }),
          });
        }}
      />
    </>
  );

  return {
    bring,
    revert,
    busy,
    markPersonal: setMarking,
    unmarkPersonal: setUnmarking,
    dialogs,
  };
}
