/**
 * "Alterações não salvas" no topo da linha do tempo (SPEC T17): os itens adicionados, removidos
 * e atualizados e os arquivos alterados desde a última versão salva, com "Salvar versão" e, em
 * cada config, "Descartar" (P1, com confirmação).
 */
import { Save, Undo2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { ChangeSet, PackId } from '../../../lib/ipc/bindings';
import { useDiscardFile } from '../api';
import { ChangeList } from './ChangeList';
import { TimelineItem } from './TimelineItem';

export interface UnsavedItemProps {
  packId: PackId;
  changes: ChangeSet;
  /** Última versão salva (`null` antes da primeira). */
  lastVersion: string | null;
  canWrite: boolean;
  onSave: () => void;
}

export function UnsavedItem({ packId, changes, lastVersion, canWrite, onSave }: UnsavedItemProps) {
  const { t } = useTranslation('versoes');
  const discard = useDiscardFile(packId);
  const [discarding, setDiscarding] = useState<string | null>(null);
  const count = changes.files.length;
  // Uma config que não existia na última versão salva é apagada ao descartar; as outras voltam
  // a ser como eram (as apagadas desde então voltam a existir).
  const discardingIsNew =
    discarding !== null &&
    changes.files.some((file) => file.path === discarding && file.kind === 'added');

  if (count === 0) {
    return (
      <li className="tl-empty t-sm t-3">
        {lastVersion ? t('naoSalvas.nada', { version: lastVersion }) : t('naoSalvas.nadaInicio')}
      </li>
    );
  }

  // Só dá para descartar o que tem para onde voltar: uma versão salva.
  const canDiscard = canWrite && lastVersion !== null;

  return (
    <>
      <TimelineItem
        kind="unsaved"
        version={t('agora')}
        summary={
          lastVersion
            ? t('naoSalvas.resumo', { count, version: lastVersion })
            : t('naoSalvas.resumoInicio', { count })
        }
        actions={
          <Button variant="primary" size="sm" icon={Save} disabled={!canWrite} onClick={onSave}>
            {t('naoSalvas.salvar')}
          </Button>
        }
        detail={
          <ChangeList
            changes={changes}
            renderConfigAction={
              canDiscard
                ? (path) => (
                    <Button
                      variant="link"
                      icon={Undo2}
                      aria-label={t('naoSalvas.descartarRotulo', { path })}
                      onClick={() => {
                        setDiscarding(path);
                      }}
                    >
                      {t('naoSalvas.descartar')}
                    </Button>
                  )
                : undefined
            }
          />
        }
        open
      />
      <ConfirmDialog
        open={discarding !== null}
        onOpenChange={(open) => {
          if (!open) {
            setDiscarding(null);
          }
        }}
        title={t('descartar.titulo', { path: discarding ?? '' })}
        description={discardingIsNew ? t('descartar.textoNovo') : t('descartar.textoExistia')}
        confirmLabel={t('descartar.confirmar')}
        confirmingLabel={t('descartar.descartando')}
        onConfirm={async () => {
          if (discarding === null) {
            return;
          }
          await discard.mutateAsync(discarding);
          showToast({ kind: 'ok', title: t('descartar.feito') });
        }}
      />
    </>
  );
}
