/**
 * Etapa "Mods iniciais" (SPEC T03; CA-T03-07 e CA-T03-08, lado da interface): as duas
 * ferramentas marcadas com a versão e para que servem, o spark antigo da CurseForge com aviso e
 * desabilitado sem chave, o kit desmarcado, o erro sem internet que deixa seguir e o vanilla.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';

import type { Kit, Loader } from '../../../../lib/ipc/bindings';
import { axeComponent } from '../../../../test/axe';
import { ipcError, mockBackend } from '../../../../test/backend';
import { makeAppError } from '../../../../test/factories';
import { renderWithProviders } from '../../../../test/render';
import { InitialModsStep } from './InitialModsStep';
import { FABRIC_KIT, FABRIC_OFFER, OLD_OFFER } from './initial-mods.fixtures';
import type { InitialChoice } from './model';

interface Setup {
  minecraft?: string;
  loader?: Loader | null;
  offer?: unknown;
  kits?: Kit[];
}

/** A etapa com o rascunho guardado, como o assistente faz. */
function Harness({
  minecraft,
  loader,
  onChoice,
}: {
  minecraft: string;
  loader: Loader | null;
  onChoice: (choice: InitialChoice | null) => void;
}) {
  const [choice, setChoice] = useState<InitialChoice | null>(null);
  return (
    <InitialModsStep
      minecraft={minecraft}
      loader={loader}
      choice={choice}
      onChange={(next) => {
        setChoice(next);
        onChoice(next);
      }}
    />
  );
}

function setup({
  minecraft = '1.21.1',
  loader = 'fabric',
  offer = FABRIC_OFFER,
  kits = [FABRIC_KIT],
}: Setup = {}) {
  const seen: (InitialChoice | null)[] = [];
  const backend = mockBackend({
    initial_mods_offer: () => (offer instanceof Error ? ipcError(offerError()) : offer),
    kits_list: () => kits,
  });
  renderWithProviders(
    <Harness
      minecraft={minecraft}
      loader={loader}
      onChoice={(choice) => {
        seen.push(choice);
      }}
    />,
  );
  return { backend, seen, last: () => seen[seen.length - 1] };
}

function offerError() {
  return makeAppError(
    { domain: 'project', code: 'SEARCH_SOURCE_UNAVAILABLE' },
    { params: { source: 'Modrinth' }, retryable: true },
  );
}

describe('etapa Mods iniciais', () => {
  it('Fabric 1.21.1: spark e Crash Assistant marcados, com a versão, para que servem e a Fabric API junto; axe', async () => {
    const { backend, last } = setup();
    const sparkBox = await screen.findByRole('checkbox', { name: 'Adicionar spark' });
    const crashBox = screen.getByRole('checkbox', { name: 'Adicionar Crash Assistant' });
    await waitFor(() => {
      expect((sparkBox as HTMLInputElement).checked).toBe(true);
    });
    expect((crashBox as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText('1.10.109-fabric')).toBeDefined();
    expect(screen.getByText('1.11.12')).toBeDefined();
    expect(
      screen.getByText('Mede o que deixa o jogo lento. Lado: cliente e servidor.'),
    ).toBeDefined();
    expect(screen.getByText(/O envio de dados ao autor do mod vem desligado/)).toBeDefined();
    expect(screen.getByText('Traz junto: Fabric API')).toBeDefined();
    expect(screen.getByText('Recomendados para Minecraft 1.21.1 com Fabric')).toBeDefined();
    expect(backend.callsOf('initial_mods_offer')[0]?.args).toEqual({
      minecraft: '1.21.1',
      loader: 'fabric',
    });
    expect(last()).toEqual({ tools: ['spark', 'crash-assistant'], kit: null });
    expect(await axeComponent(document.body)).toHaveNoViolations();
  });

  it('desmarcar uma ferramenta tira só ela da escolha', async () => {
    const user = userEvent.setup();
    const { last } = setup();
    const crash = await screen.findByRole('checkbox', { name: 'Adicionar Crash Assistant' });
    await waitFor(() => {
      expect(last()?.tools).toHaveLength(2);
    });
    await user.click(crash);
    expect(last()).toEqual({ tools: ['spark'], kit: null });
  });

  it('CA-T03-08: Forge 1.12.2 sem chave da CurseForge: spark antigo desabilitado com o motivo e o aviso; o Crash Assistant segue', async () => {
    const { last } = setup({ minecraft: '1.12.2', loader: 'forge', offer: OLD_OFFER, kits: [] });
    const spark = await screen.findByRole('checkbox', { name: 'Adicionar spark' });
    expect((spark as HTMLInputElement).disabled).toBe(true);
    expect((spark as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText('1.6.3')).toBeDefined();
    expect(screen.getByText('Versão antiga, sem atualizações')).toBeDefined();
    expect(screen.getByText('Precisa da chave da CurseForge')).toBeDefined();
    await waitFor(() => {
      expect(last()).toEqual({ tools: ['crash-assistant'], kit: null });
    });
    const crash = screen.getByRole('checkbox', { name: 'Adicionar Crash Assistant' });
    expect((crash as HTMLInputElement).disabled).toBe(false);
    expect(
      screen.getByText('Sem kit de desempenho para esta versão do Minecraft e este loader.'),
    ).toBeDefined();
  });

  it('o kit vem desmarcado; marcar mostra os mods com o C2ME desmarcado e avisado; dá para mudar', async () => {
    const user = userEvent.setup();
    const { last } = setup();
    const kitBox = await screen.findByRole('checkbox', {
      name: 'Adicionar o kit Desempenho para Fabric 1.21 em diante',
    });
    expect((kitBox as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText(/Sodium, Lithium, FerriteCore e mais 2/)).toBeDefined();
    await waitFor(() => {
      expect(last()?.tools).toHaveLength(2);
    });

    await user.click(kitBox);
    expect(last()?.kit).toEqual({
      id: 'fabric-moderno',
      projects: ['AANobbMI', 'gvQqBUqZ', 'uXXizFIs', '5ZwdcRci'],
    });
    const list = screen.getByRole('list', {
      name: 'Mods do kit Desempenho para Fabric 1.21 em diante',
    });
    const c2me = within(list).getByRole('checkbox', { name: 'Incluir C2ME' });
    expect((c2me as HTMLInputElement).checked).toBe(false);
    expect(within(list).getByText('Muda a geração do mundo. Vem desmarcado.')).toBeDefined();
    await user.click(within(list).getByRole('checkbox', { name: 'Incluir Sodium' }));
    await user.click(c2me);
    expect(last()?.kit?.projects).toEqual(['gvQqBUqZ', 'uXXizFIs', '5ZwdcRci', 'VSNURh3q']);

    await user.click(kitBox);
    expect(last()?.kit).toBeNull();
  });

  it('sem internet: avisa, deixa seguir sem os mods e tenta de novo', async () => {
    const user = userEvent.setup();
    let online = false;
    const seen: (InitialChoice | null)[] = [];
    mockBackend({
      initial_mods_offer: () => (online ? FABRIC_OFFER : ipcError(offerError())),
      kits_list: () => [],
    });
    renderWithProviders(
      <Harness
        minecraft="1.21.1"
        loader="fabric"
        onChoice={(choice) => {
          seen.push(choice);
        }}
      />,
    );
    expect(
      await screen.findByText('Não foi possível consultar os mods iniciais agora'),
    ).toBeDefined();
    await waitFor(() => {
      expect(seen.at(-1)).toEqual({ tools: [], kit: null });
    });
    online = true;
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByRole('checkbox', { name: 'Adicionar spark' })).toBeDefined();
  });

  it('vanilla: não consulta nada e explica', async () => {
    const { backend, last } = setup({ loader: null, kits: [] });
    expect(await screen.findByText('Este pack não tem loader')).toBeDefined();
    await waitFor(() => {
      expect(last()).toEqual({ tools: [], kit: null });
    });
    expect(backend.callsOf('initial_mods_offer')).toHaveLength(0);
  });
});
