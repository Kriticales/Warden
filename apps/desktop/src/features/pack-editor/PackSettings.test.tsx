/**
 * Informações do pack e Ajustes do teste neste computador (T11), abertos pelo cabeçalho.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { JavaChoice } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { renderApp } from '../../test/render';
import { DECISIONS, makeChoice, makeMeta, makeTestSettingsView } from './editor.fixtures';
import { editorBackend } from './testing';

async function openPack(url: string) {
  renderApp(url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
}

describe('Informações do pack (T11)', () => {
  it('CA-T11-01 (interface): muda o nome e grava nome, autor e descrição do pack.toml', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        pack_meta_get: () => makeMeta({ author: 'Kriticales', description: 'Pack calmo.' }),
        pack_update_meta: ({ update }) => makeMeta(update as Record<string, string>),
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Editar informações' }));
    const dialog = await screen.findByRole('dialog', { name: 'Informações do pack' });
    const name = await within(dialog).findByRole<HTMLInputElement>('textbox', { name: 'Nome' });
    expect(name.value).toBe('Vale Sereno');
    expect(within(dialog).getByRole<HTMLInputElement>('textbox', { name: 'Autor' }).value).toBe(
      'Kriticales',
    );
    expect(dialog.textContent).toContain('Minecraft 1.20.1 · Forge 47.3.0');
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.clear(name);
    await user.type(name, '  Vale Novo ');
    await user.click(within(dialog).getByRole('button', { name: 'Salvar' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_update_meta')).toHaveLength(1);
    });
    expect(backend.callsOf('pack_update_meta')[0]?.args).toMatchObject({
      update: { name: 'Vale Novo', author: 'Kriticales', description: 'Pack calmo.' },
    });
    expect(await screen.findByText('Informações do pack salvas.')).toBeDefined();
  });

  it('nome vazio não grava e diz o que falta', async () => {
    const { backend, url } = editorBackend();
    await openPack(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Editar informações' }));
    const dialog = await screen.findByRole('dialog', { name: 'Informações do pack' });
    await user.clear(await within(dialog).findByRole('textbox', { name: 'Nome' }));
    await user.click(within(dialog).getByRole('button', { name: 'Salvar' }));
    expect(within(dialog).getByText('Digite o nome do pack.')).toBeDefined();
    expect(backend.callsOf('pack_update_meta')).toHaveLength(0);
  });
});

/** Abre Ajustes do teste pelo menu ▾ do Testar. */
async function openTestSettings(choice: JavaChoice, pack: Record<string, unknown> = {}) {
  const setup = editorBackend({
    pack,
    handlers: {
      java_choice: () => choice,
      pack_test_settings_set: ({ settings }) =>
        makeTestSettingsView({ settings: settings as never }),
    },
  });
  await openPack(setup.url);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Mais opções do teste' }));
  await user.click(
    await screen.findByRole('menuitem', { name: /Ajustes do teste neste computador/ }),
  );
  const dialog = await screen.findByRole('dialog', { name: 'Ajustes do teste neste computador' });
  return { ...setup, user, dialog };
}

describe('Ajustes do teste neste computador (T11)', () => {
  it('CA-T11-03: Forge 1.20.1 → "Automático: Java 17" e por que não o Java mais novo; axe', async () => {
    const { dialog, backend } = await openTestSettings(makeChoice(DECISIONS.forge1201));
    expect(
      await within(dialog).findByRole('option', { name: 'Automático: Java 17' }),
    ).toBeDefined();
    expect(within(dialog).getByText('Por que não o Java 25?')).toBeDefined();
    expect(dialog.textContent).toContain(
      'O Minecraft 1.17 a 1.20.4 foi feito para o Java 17. Um Java mais novo ainda não foi provado com essas versões.',
    );
    expect(backend.callsOf('java_choice')[0]?.args).toMatchObject({
      request: {
        minecraft: '1.20.1',
        loader: 'forge',
        loaderVersion: '47.3.0',
        versionJson: { kind: 'notLoaded' },
        userChoice: null,
      },
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T11-03: Forge 1.7.10 → "Automático: Java 8" com o motivo do Forge antigo', async () => {
    const { dialog } = await openTestSettings(makeChoice(DECISIONS.forge1710), {
      minecraft: '1.7.10',
      loaderVersion: '10.13.4.1614',
    });
    expect(await within(dialog).findByRole('option', { name: 'Automático: Java 8' })).toBeDefined();
    expect(within(dialog).getByText('Por que não o Java 25?')).toBeDefined();
    expect(dialog.textContent).toContain(
      'O Forge 1.7.10 e 1.12.2 só abre no Java 8. A partir do Java 9 ele trava ao iniciar.',
    );
  });

  it('CA-T11-03: a versão mais nova do Minecraft → o Java mais novo, sem explicação extra', async () => {
    const { dialog } = await openTestSettings(makeChoice(DECISIONS.newest), {
      minecraft: '26.1',
      loader: null,
      loaderVersion: null,
    });
    expect(
      await within(dialog).findByRole('option', { name: 'Automático: Java 25' }),
    ).toBeDefined();
    expect(within(dialog).queryByText(/Por que não o Java/)).toBeNull();
  });

  it('grava memória e argumentos em packs.json, com aviso dos argumentos', async () => {
    const { dialog, backend, user } = await openTestSettings(makeChoice(DECISIONS.forge1201));
    await within(dialog).findByRole('option', { name: 'Automático: Java 17' });
    await user.selectOptions(
      within(dialog).getByRole('combobox', { name: 'Memória do teste' }),
      '6144',
    );
    const args = within(dialog).getByRole('textbox', { name: /Argumentos extras da JVM/ });
    await user.type(args, '-XX:+UseG1GC -XX:+UseConcMarkSweepGC');
    expect(dialog.textContent).toContain(
      '-XX:+UseConcMarkSweepGC: não existe mais no Java 17. O jogo pode não abrir.',
    );
    await user.click(within(dialog).getByRole('button', { name: 'Salvar ajustes' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_test_settings_set')).toHaveLength(1);
    });
    expect(backend.callsOf('pack_test_settings_set')[0]?.args).toMatchObject({
      settings: {
        memory: { mode: 'fixed', mb: 6144 },
        java: null,
        jvmArgs: '-XX:+UseG1GC -XX:+UseConcMarkSweepGC',
      },
    });
    expect(await screen.findByText('Ajustes do teste salvos.')).toBeDefined();
  });

  it('o botão Testar principal ainda não testa e diz por quê', async () => {
    const { url } = editorBackend();
    await openPack(url);
    const test = screen.getByRole('button', { name: 'Testar' });
    expect(test.getAttribute('aria-disabled')).toBe('true');
  });
});
