/**
 * As correções de um achado (SPEC T14: "Correções passam pelos fluxos normais, com
 * confirmação"). Nenhuma grava nada sozinha:
 * - **Adicionar dependência**: acha o projeto na fonte e abre o diálogo de dependências de
 *   sempre (T09); sem projeto conhecido, leva a Adicionar;
 * - **Remover**: a confirmação de Remover da lista de Mods (com quem depende do item);
 * - **Mudar lado**: o mesmo "Alterar lado" da lista de Mods;
 * - **Atualizar**: abre os detalhes do item em Mods (troca de versão e atualização);
 * - **Java, memória, argumento**: abre Ajustes do teste neste computador;
 * - **Config**: abre o arquivo em Configs;
 * - **Restaurar padrão** (C-06): a diferença da chave restaurada e a gravação com confirmação.
 *
 * `useFixes` devolve a ação e os diálogos, que a tela põe uma vez (`fixes.dialogs`).
 */
import { useNavigate } from '@tanstack/react-router';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { showToast } from '../../components/ui/toast';
import type {
  AddChoice,
  InventoryItem,
  PackId,
  ReportFinding,
  SourceId,
  SuggestedFix,
} from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { DependenciesDialog } from '../add/dependencies/DependenciesDialog';
import {
  PackRestoreDialog,
  type PackRestoreTarget,
} from '../configs/form/defaults/PackRestoreDialog';
import { useInventory, useSetSide } from '../pack-editor/api';
import { RemoveDialog } from '../pack-editor/mods/RemoveDialog';
import { TestSettingsDialog } from '../pack-editor/test-settings/TestSettingsDialog';
import { usePack } from '../packs/api';
import { useFindDependency } from './api';
import { itemOf, refName } from './text';

export interface Fixes {
  /** Começa a correção (diálogo, navegação ou gravação com confirmação). */
  run: (entry: ReportFinding, fix: SuggestedFix) => void;
  /** A correção em andamento (para o botão mostrar que está ocupado). */
  busy: SuggestedFix | null;
  /** Os diálogos das correções: a tela põe uma vez. */
  dialogs: ReactNode;
}

/** A fonte da dependência pelo achado: a da API que a declarou, ou nenhuma (id do jar). */
function dependencySource(entry: ReportFinding): SourceId | null {
  for (const evidence of entry.finding.evidence) {
    if (evidence.kind === 'api') {
      return evidence.source === 'curseForge' ? 'curseforge' : 'modrinth';
    }
  }
  return null;
}

export function useFixes(
  packId: PackId,
  /** Depois de uma correção que mudou o pack (o chamador verifica de novo, testa de novo…). */
  onFixed?: () => void,
): Fixes {
  const { t } = useTranslation('diagnostico');
  const navigate = useNavigate();
  const pack = usePack(packId);
  const inventory = useInventory(packId);
  const findDependency = useFindDependency(packId);
  const setSide = useSetSide(packId);
  const [busy, setBusy] = useState<SuggestedFix | null>(null);
  const [adding, setAdding] = useState<{ choices: AddChoice[]; titles: string[] } | null>(null);
  const [removing, setRemoving] = useState<InventoryItem[] | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [restoring, setRestoring] = useState<PackRestoreTarget | null>(null);

  const fail = (error: unknown) => {
    showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
  };

  const run = (entry: ReportFinding, fix: SuggestedFix) => {
    switch (fix.kind) {
      case 'addDependency': {
        const name = entry.labels[fix.modId] ?? fix.modId;
        setBusy(fix);
        findDependency.mutate(
          { modId: fix.modId, source: dependencySource(entry) },
          {
            onSuccess: (candidate) => {
              if (candidate) {
                setAdding({
                  choices: [{ source: candidate.source, projectId: candidate.projectId }],
                  titles: [candidate.name],
                });
              } else {
                showToast({
                  kind: 'warn',
                  title: t('correcao.adicionarNaoAchou', { nome: name }),
                  action: {
                    label: t('correcao.abrirAdicionar'),
                    altText: t('correcao.abrirAdicionar'),
                    onClick: () => {
                      void navigate({ to: '/packs/$packId/adicionar', params: { packId } });
                    },
                  },
                });
              }
            },
            onError: fail,
            onSettled: () => {
              setBusy(null);
            },
          },
        );
        return;
      }
      case 'removeItem': {
        const path = itemOf(entry, fix.item)?.path;
        const item = inventory.data?.items.find((candidate) => candidate.path === path);
        if (item) setRemoving([item]);
        return;
      }
      case 'setSide': {
        const path = itemOf(entry, fix.item)?.path;
        if (!path) return;
        setBusy(fix);
        setSide.mutate(
          { paths: [path], side: fix.side },
          {
            onSuccess: () => {
              showToast({
                kind: 'ok',
                title: t('correcao.ladoFeito', { nome: refName(entry, fix.item) }),
              });
              onFixed?.();
            },
            onError: fail,
            onSettled: () => {
              setBusy(null);
            },
          },
        );
        return;
      }
      case 'updateItem': {
        const path = itemOf(entry, fix.item)?.path;
        if (path) {
          void navigate({ to: '/packs/$packId/mods', params: { packId }, search: { item: path } });
        }
        return;
      }
      case 'changeJava':
      case 'changeMemory':
      case 'removeJvmArgument':
        setSettingsOpen(true);
        return;
      case 'restoreDefault':
        setRestoring({ path: fix.path, key: fix.key });
        return;
      case 'restoreConfig':
        void navigate({
          to: '/packs/$packId/configs',
          params: { packId },
          search: { arquivo: fix.path },
        });
        return;
    }
  };

  const target = {
    minecraft: pack.data?.minecraft ?? '',
    loader: pack.data?.loader ?? null,
  };
  const dialogs = (
    <>
      <DependenciesDialog
        packId={packId}
        request={adding}
        target={target}
        onClose={() => {
          setAdding(null);
        }}
        onAdded={() => {
          setAdding(null);
          showToast({ kind: 'ok', title: t('ajuda.corrigido') });
          onFixed?.();
        }}
      />
      <RemoveDialog
        packId={packId}
        items={removing}
        onClose={() => {
          setRemoving(null);
        }}
        onRemoved={() => {
          setRemoving(null);
          onFixed?.();
        }}
      />
      <PackRestoreDialog
        packId={packId}
        target={restoring}
        onClose={() => {
          setRestoring(null);
        }}
        onRestored={onFixed}
      />
      {pack.data ? (
        <TestSettingsDialog
          packId={packId}
          pack={pack.data}
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
        />
      ) : null}
    </>
  );
  return { run, busy, dialogs };
}
