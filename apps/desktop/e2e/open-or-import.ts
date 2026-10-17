/**
 * Ajuda comum dos E2E: "Abrir ou importar…" de Meus packs (T04, T24) é um botão só, com
 * **Escolher pasta…** e **Escolher arquivo…** num menu. Nos E2E, o diálogo nativo é trocado pelo
 * caminho escrito no arquivo de `WARDEN_E2E_PICK_FOLDER`; aqui o teste só abre o menu e escolhe.
 */
import { $ } from '@wdio/globals';

export async function openOrImport(choice: 'pasta' | 'arquivo' = 'pasta'): Promise<void> {
  const label = choice === 'pasta' ? 'Escolher pasta…' : 'Escolher arquivo…';
  await $('button=Abrir ou importar…').click();
  const item = $(`[role="menuitem"]*=${label}`);
  await item.waitForClickable();
  await item.click();
}
