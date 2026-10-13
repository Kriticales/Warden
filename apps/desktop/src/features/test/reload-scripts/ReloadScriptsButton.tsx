/**
 * "Recarregar scripts" na tela do jogo aberto (SPEC T13 passo 6 e T26; protótipo `teste-jogo`):
 * só aparece quando o pack tem KubeJS ou CraftTweaker. Recarrega os scripts de servidor pelo
 * mesmo caminho do editor de scripts (servidor web do KubeJS 7, console do servidor local ou o
 * comando para copiar, que aqui vem num aviso com **Copiar**).
 */
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type { PackId } from '../../../lib/ipc/bindings';
import { useScriptsInfo } from '../../configs/scripts/api';
import { reloadTargetPath, toolFor, toolName } from '../../configs/scripts/model';
import { useReloadAction, type ReloadNote } from '../../configs/scripts/reload';

export function ReloadScriptsButton({ packId }: { packId: PackId }) {
  const { t } = useTranslation('scripts');
  const info = useScriptsInfo(packId, true);
  const env = info.data?.env;
  const target = env ? reloadTargetPath(env) : null;
  const kind = env?.kubejs ? 'kubejs' : 'crafttweaker';
  const tool = env ? toolFor(env, kind) : null;
  const name = tool ? toolName(kind, tool) : '';
  const reload = useReloadAction(packId, (note: ReloadNote) => {
    if (note?.kind === 'copy') {
      showToast({
        kind: 'info',
        title: t('comando.titulo', { ferramenta: name }),
        text: t('comando.texto'),
        action: {
          label: t('comando.copiar', { comando: note.command }),
          altText: t('teste.altCopiar'),
          onClick: () => {
            void navigator.clipboard.writeText(note.command).then(() => {
              showToast({ kind: 'ok', title: t('comando.copiado') });
            });
          },
        },
      });
    } else if (note?.kind === 'restart') {
      showToast({ kind: 'warn', title: t('reiniciar.titulo'), text: t('reiniciar.texto') });
    }
  });
  if (!target) {
    return null;
  }
  return (
    <Tooltip content={t('teste.dica', { ferramenta: name })}>
      <Button
        icon={RefreshCw}
        loading={reload.pending}
        onClick={() => {
          void reload.run(target);
        }}
      >
        {t('teste.recarregar')}
      </Button>
    </Tooltip>
  );
}
