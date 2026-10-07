/**
 * Mods opcionais e fixar versão (T11; P1-14), no app inteiro com o backend simulado: "Mais
 * opções" do painel de detalhes e "Mods opcionais" dos Ajustes do teste.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { InventoryItem, OptionalChoice } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { AE2, INVALID, makeDetails, makeItem, makeInventory, SODIUM } from '../editor.fixtures';
import { editorBackend } from '../testing';

const OPTIONAL_MOD = makeItem({
  name: 'Dynamic Lights',
  path: 'mods/dynamic-lights.pw.toml',
  optional: true,
});

async function openDetails(item: InventoryItem, options: Parameters<typeof editorBackend>[0]) {
  const setup = editorBackend({
    ...options,
    inventory: makeInventory([item]),
    handlers: { item_details: () => makeDetails(item), ...options?.handlers },
  });
  renderApp(setup.url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: item.name }));
  const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
  await user.click(within(drawer).getByText('Mais opções'));
  return { ...setup, drawer, user };
}

describe('Mais opções do item (T11)', () => {
  it('Fixar versão grava pin pelo caminho do item e solta de volta', async () => {
    const { backend, drawer, user } = await openDetails(AE2, {
      handlers: { items_set_pinned: () => [AE2.path] },
    });
    const pin = await within(drawer).findByRole('switch', { name: 'Fixar versão (não atualizar)' });
    expect(pin.getAttribute('aria-checked')).toBe('false');
    await user.click(pin);
    await waitFor(() => {
      expect(backend.callsOf('items_set_pinned')).toHaveLength(1);
    });
    expect(backend.callsOf('items_set_pinned')[0]?.args).toMatchObject({
      paths: [AE2.path],
      pinned: true,
    });
    expect(await screen.findByText('Versão fixada.')).toBeDefined();
  });

  it('um item já fixado mostra o interruptor ligado e solta com um clique', async () => {
    const { backend, drawer, user } = await openDetails(SODIUM, {
      handlers: { items_set_pinned: () => [SODIUM.path] },
    });
    const pin = await within(drawer).findByRole('switch', { name: 'Fixar versão (não atualizar)' });
    expect(pin.getAttribute('aria-checked')).toBe('true');
    await user.click(pin);
    await waitFor(() => {
      expect(backend.callsOf('items_set_pinned')).toHaveLength(1);
    });
    expect(backend.callsOf('items_set_pinned')[0]?.args).toMatchObject({ pinned: false });
  });

  it('marcar como opcional grava [option] ligado por padrão e mostra o aviso dos outros formatos; axe', async () => {
    const { backend, drawer, user } = await openDetails(AE2, {
      handlers: { item_set_optional: () => [AE2.path] },
    });
    const optional = await within(drawer).findByRole('switch', { name: 'Opcional para o jogador' });
    expect(optional.getAttribute('aria-checked')).toBe('false');
    expect(drawer.textContent).toContain(
      'O app do Modrinth instala todos os opcionais; o formato da CurseForge não suporta lado.',
    );
    expect(within(drawer).queryByRole('textbox', { name: /Descrição para o jogador/ })).toBeNull();
    await user.click(optional);
    await waitFor(() => {
      expect(backend.callsOf('item_set_optional')).toHaveLength(1);
    });
    expect(backend.callsOf('item_set_optional')[0]?.args).toMatchObject({
      path: AE2.path,
      settings: { description: '', default: true },
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('item opcional: edita a descrição e o padrão e salva só o que mudou', async () => {
    const { backend, drawer, user } = await openDetails(OPTIONAL_MOD, {
      handlers: {
        item_option_get: () => ({ description: 'Luz nas mãos', default: true }),
        item_set_optional: () => [OPTIONAL_MOD.path],
      },
    });
    const description = await within(drawer).findByRole<HTMLInputElement>('textbox', {
      name: /Descrição para o jogador/,
    });
    expect(description.value).toBe('Luz nas mãos');
    const save = within(drawer).getByRole('button', { name: 'Salvar' });
    expect(save.hasAttribute('disabled')).toBe(true);
    await user.clear(description);
    await user.type(description, '  Tochas que iluminam  ');
    await user.click(within(drawer).getByRole('switch', { name: 'Ligado por padrão' }));
    await user.click(save);
    await waitFor(() => {
      expect(backend.callsOf('item_set_optional')).toHaveLength(1);
    });
    expect(backend.callsOf('item_set_optional')[0]?.args).toMatchObject({
      path: OPTIONAL_MOD.path,
      settings: { description: 'Tochas que iluminam', default: false },
    });
  });

  it('descrição longa demais não grava e diz o limite', async () => {
    const { backend, drawer, user } = await openDetails(OPTIONAL_MOD, {
      handlers: { item_option_get: () => ({ description: '', default: false }) },
    });
    const description = await within(drawer).findByRole('textbox', {
      name: /Descrição para o jogador/,
    });
    await user.click(description);
    await user.paste('a'.repeat(301));
    expect(within(drawer).getByText('Use no máximo 300 caracteres.')).toBeDefined();
    expect(within(drawer).getByRole('button', { name: 'Salvar' }).hasAttribute('disabled')).toBe(
      true,
    );
    expect(backend.callsOf('item_set_optional')).toHaveLength(0);
  });

  it('desmarcar remove o [option] (settings nulo)', async () => {
    const { backend, drawer, user } = await openDetails(OPTIONAL_MOD, {
      handlers: {
        item_option_get: () => ({ description: '', default: true }),
        item_set_optional: () => [OPTIONAL_MOD.path],
      },
    });
    const optional = await within(drawer).findByRole('switch', { name: 'Opcional para o jogador' });
    await waitFor(() => {
      expect(optional.getAttribute('aria-checked')).toBe('true');
    });
    await user.click(optional);
    await waitFor(() => {
      expect(backend.callsOf('item_set_optional')).toHaveLength(1);
    });
    expect(backend.callsOf('item_set_optional')[0]?.args).toMatchObject({ settings: null });
  });

  it('erro do domínio aparece no próprio bloco', async () => {
    const { drawer, user } = await openDetails(AE2, {
      handlers: {
        items_set_pinned: () =>
          ipcError(makeAppError({ domain: 'project', code: 'PACK_CHANGED_EXTERNALLY' })),
      },
    });
    await user.click(
      await within(drawer).findByRole('switch', { name: 'Fixar versão (não atualizar)' }),
    );
    expect(await within(drawer).findByRole('alert')).toBeDefined();
  });

  it('arquivo inválido não tem Mais opções', async () => {
    const setup = editorBackend({ inventory: makeInventory([INVALID]) });
    renderApp(setup.url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    await userEvent.setup().click(await screen.findByRole('button', { name: INVALID.path }));
    await screen.findByRole('dialog');
    expect(screen.queryByText('Mais opções')).toBeNull();
  });
});

const CHOICES: OptionalChoice[] = [
  {
    path: 'mods/shaders.pw.toml',
    name: 'Iris Shaders',
    description: 'Sombras bonitas',
    default: false,
    enabled: false,
  },
  {
    path: 'mods/minimapa.pw.toml',
    name: 'Xaero Minimap',
    description: '',
    default: true,
    enabled: true,
  },
];

async function openTestSettings(handlers: Parameters<typeof editorBackend>[0]) {
  const setup = editorBackend(handlers);
  renderApp(setup.url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Mais opções do teste' }));
  await user.click(await screen.findByRole('menuitem', { name: /Ajustes do teste neste computador/ }));
  const dialog = await screen.findByRole('dialog', { name: 'Ajustes do teste neste computador' });
  return { ...setup, dialog, user };
}

describe('Mods opcionais nos Ajustes do teste (T11)', () => {
  it('lista os opcionais com a escolha da instância e grava todas as escolhas ao ligar um; axe', async () => {
    const { backend, dialog, user } = await openTestSettings({
      handlers: {
        instance_optional_choices_get: () => CHOICES,
        instance_set_optional_choices: () => null,
      },
    });
    const iris = await within(dialog).findByRole('switch', { name: 'Iris Shaders' });
    expect(iris.getAttribute('aria-checked')).toBe('false');
    expect(
      within(dialog).getByRole('switch', { name: 'Xaero Minimap' }).getAttribute('aria-checked'),
    ).toBe('true');
    expect(dialog.textContent).toContain('Sombras bonitas. Padrão do pack: desligado');
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(iris);
    await waitFor(() => {
      expect(backend.callsOf('instance_set_optional_choices')).toHaveLength(1);
    });
    expect(backend.callsOf('instance_set_optional_choices')[0]?.args).toMatchObject({
      choices: { 'mods/shaders.pw.toml': true, 'mods/minimapa.pw.toml': true },
    });
    // Escolher opcional da instância não toca no pack.
    expect(backend.callsOf('items_set_pinned')).toHaveLength(0);
    expect(backend.callsOf('item_set_optional')).toHaveLength(0);
  });

  it('pack sem opcionais diz que não há nenhum', async () => {
    const { dialog } = await openTestSettings({});
    expect(await within(dialog).findByText('Este pack não tem mods opcionais.')).toBeDefined();
  });
});
