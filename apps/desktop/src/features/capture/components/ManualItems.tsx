/**
 * Mods, resource packs e shaders colocados ou tirados à mão na pasta da instância, e datapacks
 * criados nos mundos (SPEC T15 "Grupos"; CA-T15-04). Nada disso é copiado para o pack: o arquivo
 * novo é procurado pelo hash e, quando reconhecido, "Adicionar ao pack pelo Modrinth" (ou pela
 * CurseForge) abre o caminho normal de Adicionar, com as dependências.
 */
import { Plus } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import type {
  CaptureFileView,
  CaptureIdentified,
  HashMatch,
  PackId,
} from '../../../lib/ipc/bindings';
import {
  DependenciesDialog,
  type DependenciesRequest,
} from '../../add/dependencies/DependenciesDialog';
import { usePack } from '../../packs/api';
import { useCaptureIdentify } from '../api';

export interface ManualItemsProps {
  packId: PackId;
  sessionId: string;
  items: readonly CaptureFileView[];
}

/** Nome do arquivo (sem a pasta). */
function fileName(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

export function ManualItems({ packId, sessionId, items }: ManualItemsProps) {
  const pack = usePack(packId);
  const searchable = items.some(
    (item) => item.file.group !== 'datapack' && item.file.change !== 'removed',
  );
  const identify = useCaptureIdentify(packId, sessionId, searchable);
  const [request, setRequest] = useState<DependenciesRequest | null>(null);

  return (
    <>
      {items.map((item) => (
        <ManualItem
          key={item.file.path}
          view={item}
          loading={identify.isPending && searchable}
          identified={identify.data ?? null}
          failed={identify.isError}
          onAdd={(found) => {
            setRequest({
              choices: [
                { source: found.source, projectId: found.projectId, versionId: found.versionId },
              ],
              titles: [found.title],
            });
          }}
        />
      ))}
      <DependenciesDialog
        packId={packId}
        request={request}
        target={{ minecraft: pack.data?.minecraft ?? '', loader: pack.data?.loader ?? null }}
        onClose={() => {
          setRequest(null);
        }}
        onAdded={() => {
          setRequest(null);
        }}
      />
    </>
  );
}

function ManualItem({
  view,
  loading,
  identified,
  failed,
  onAdd,
}: {
  view: CaptureFileView;
  loading: boolean;
  identified: CaptureIdentified | null;
  failed: boolean;
  onAdd: (found: HashMatch) => void;
}) {
  const { t } = useTranslation('captura');
  const file = view.file;
  const arquivo = fileName(file.path);
  let text: string;
  let action = null;
  if (file.group === 'datapack') {
    text = t('mod.datapack', { arquivo: file.path });
  } else if (file.change === 'removed') {
    text = t('mod.removido', { arquivo });
  } else if (loading) {
    text = t('mod.procurando', { arquivo });
  } else {
    const found = identified?.items.find((item) => item.path === file.path)?.found ?? null;
    const unavailable = failed || (identified?.failed.length ?? 0) > 0;
    if (found) {
      const fonte = t(`fontes.${found.source}`);
      text = t('mod.reconhecido', {
        arquivo,
        nome: found.title,
        versao: found.versionNumber,
        fonte,
      });
      if (found.kind !== null) {
        action = (
          <Button
            size="sm"
            icon={Plus}
            onClick={() => {
              onAdd(found);
            }}
          >
            {t('mod.adicionar', { fonte })}
          </Button>
        );
      }
    } else {
      text = unavailable ? t('mod.semBusca', { arquivo }) : t('mod.naoReconhecido', { arquivo });
    }
  }
  return (
    <div className="change-item change-item--info">
      <div className="t-sm">
        <span className="path">{file.path}</span>
        <p className="t-2">{text}</p>
      </div>
      <div>{action}</div>
    </div>
  );
}
