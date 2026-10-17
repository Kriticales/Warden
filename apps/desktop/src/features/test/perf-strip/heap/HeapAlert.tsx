/**
 * Os avisos da memória do jogo abaixo da faixa (SPEC T13; L-11; protótipo `teste-jogo`, estados
 * "Memória alta" e "Sem leitura de memória"):
 *
 * - **memória quase cheia** (o `W_LOW_HEAP` ao vivo): depois das coletas, mais de 90% do máximo
 *   há mais de 1 minuto; diz para quanto aumentar e abre Ajustes do teste;
 * - **sem leitura**: o motivo; com um argumento que desliga a leitura, abre Ajustes do teste,
 *   que oferece removê-lo (CA-T13-10).
 *
 * Ajustes do teste é o diálogo de sempre (P1-08/L-08): o que mudar vale no próximo teste. Ao
 * fechar, o foco volta ao botão que o abriu.
 */
import { Settings } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../../components/ui/alert';
import { Button } from '../../../../components/ui/button';
import type { PackId } from '../../../../lib/ipc/bindings';
import { TestSettingsDialog } from '../../../pack-editor/test-settings/TestSettingsDialog';
import { usePack } from '../../../packs/api';
import { formatDuration, formatMemory } from '../../text';
import type { HeapView } from './model';

/** O argumento das "flags do Aikar" que desliga a leitura (o texto cita a origem). */
const AIKAR_FLAG = '-XX:+PerfDisableSharedMem';

/** "Ajustes do teste…" com o diálogo. */
function SettingsButton({ packId }: { packId: PackId }) {
  const { t } = useTranslation('memoria');
  const pack = usePack(packId);
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        size="sm"
        icon={Settings}
        onClick={() => {
          setOpen(true);
        }}
      >
        {t('aviso.ajustes')}
      </Button>
      {pack.data ? (
        <TestSettingsDialog packId={packId} pack={pack.data} open={open} onOpenChange={setOpen} />
      ) : null}
    </>
  );
}

export function HeapAlert({ packId, view }: { packId: PackId; view: HeapView }) {
  const { t } = useTranslation('memoria');
  if (view.kind === 'reading') {
    const { heap } = view;
    const seconds = heap.details.lowHeapS;
    if (seconds === null) return null;
    return (
      <Alert
        kind="warn"
        title={t('aviso.cheiaTitulo')}
        actions={<SettingsButton packId={packId} />}
      >
        {t('aviso.cheiaTexto', {
          maximo: formatMemory(heap.maxMb),
          duracao: formatDuration(seconds * 1000),
          sugerida: formatMemory(heap.details.suggestedMb ?? heap.maxMb),
        })}
      </Alert>
    );
  }
  if (view.kind === 'unavailable') {
    const { reason } = view;
    let text: string;
    if (reason.kind === 'argument') {
      text = t(
        reason.argument === AIKAR_FLAG ? 'aviso.semLeituraAikar' : 'aviso.semLeituraArgumento',
        {
          argumento: reason.argument,
        },
      );
    } else {
      text = t('aviso.semLeituraJava');
    }
    return (
      <Alert
        kind="neutral"
        compact
        title={t('aviso.semLeituraTitulo')}
        actions={reason.kind === 'argument' ? <SettingsButton packId={packId} /> : undefined}
      >
        {text}
      </Alert>
    );
  }
  return null;
}
