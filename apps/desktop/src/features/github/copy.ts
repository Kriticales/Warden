/**
 * "Copiar link", "Copiar instruções" e "Copiar texto das notas da versão": copia e avisa com
 * um toast (o texto falhou ao copiar → o aviso diz para copiar à mão).
 */
import type { TFunction } from 'i18next';

import { showToast } from '../../components/ui/toast';
import { log } from '../../lib/log';

export async function copyText(
  t: TFunction<'publicar'>,
  text: string,
  done: 'copiar.copiado' | 'copiar.instrucoesCopiadas' | 'copiar.notasCopiadas',
): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    showToast({ kind: 'ok', title: t(done) });
  } catch (error) {
    log.warn('falha ao copiar para a área de transferência', error);
    showToast({ kind: 'danger', title: t('copiar.falhou') });
  }
}
