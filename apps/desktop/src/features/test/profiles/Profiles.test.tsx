/**
 * Perfis do teste (SPEC T11 "Perfis do teste"; ROADMAP L-08) no app inteiro com o backend
 * simulado: o seletor Perfil dos Ajustes do teste (Novo perfil, Renomear, Apagar), o grupo
 * "Perfil do teste" do menu ▾ com itens de rádio, o botão "Testar · <perfil>", "Ao abrir o jogo"
 * (Quick Play só na 1.20+), a janela, os opcionais por perfil e os avisos de memória e de
 * argumentos. Os critérios estão no nome de cada teste.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import type { OptionalChoice, PackRow, TestProfilesView } from '../../../lib/ipc/bindings';
import { axeComponent, axePage } from '../../../test/axe';
import type { Handler } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { useTestStore } from '../store';
import { makeGame } from '../test.fixtures';
import { makeProfile, makeProfilesView, profilesHandlers } from './profiles.fixtures';

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
});

interface Setup {
  pack?: Partial<PackRow>;
  profiles?: TestProfilesView;
  handlers?: Record<string, Handler>;
}

async function openPack({ pack, profiles, handlers = {} }: Setup = {}) {
  const state = profilesHandlers(profiles);
  const setup = editorBackend({ pack, handlers: { ...state.handlers, ...handlers } });
  renderApp(setup.url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  return { ...setup, view: state.view, user: userEvent.setup() };
}

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

async function openMenu(user: ReturnType<typeof userEvent.setup>) {
  await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
  return screen.findByRole('menu');
}

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  const menu = await openMenu(user);
  await user.click(
    await within(menu).findByRole('menuitem', { name: /Ajustes do teste neste computador/ }),
  );
  // O diálogo troca o "Lendo os ajustes…" pelo formulário quando os perfis chegam: só depois
  // dele o elemento do diálogo fica estável.
  await screen.findByRole('combobox', { name: 'Perfil do teste' });
  return screen.getByRole('dialog', { name: 'Ajustes do teste neste computador' });
}

/** O cabeçalho, também com um diálogo modal aberto (que esconde o resto da página). */
function hiddenHeader() {
  return screen.getByRole('banner', { name: 'Pack aberto', hidden: true });
}

const WEAK = makeProfile('perfil-1', 'PC fraco (4 GB)', { memory: { mode: 'fixed', mb: 4096 } });

describe('perfis no menu ▾ e no botão Testar (CA-T11-04)', () => {
  it('criar "PC fraco (4 GB)", escolhê-lo no menu ▾ e testar: o botão mostra "Testar · PC fraco" e o teste usa o perfil', async () => {
    const { backend, user } = await openPack({
      handlers: { test_start: () => new Promise(() => undefined) },
    });
    // Sem perfil próprio, o botão é só "Testar".
    expect(within(header()).getByRole('button', { name: 'Testar' })).toBeDefined();

    const dialog = await openSettings(user);
    await user.click(within(dialog).getByRole('button', { name: 'Novo perfil' }));
    await user.type(
      within(dialog).getByRole('textbox', { name: 'Nome do perfil' }),
      'PC fraco (4 GB)',
    );
    await user.selectOptions(
      within(dialog).getByRole('combobox', { name: 'Memória do teste' }),
      '4096',
    );
    await user.click(within(dialog).getByRole('button', { name: 'Criar perfil' }));
    await waitFor(() => {
      expect(backend.callsOf('test_profile_create')).toHaveLength(1);
    });
    expect(backend.callsOf('test_profile_create')[0]?.args).toMatchObject({
      name: 'PC fraco (4 GB)',
      fields: { memory: { mode: 'fixed', mb: 4096 }, onLaunch: { mode: 'menu' } },
    });
    // O diálogo passa a editar o perfil novo; o Testar continua no Padrão.
    const picker = within(dialog).getByRole('combobox', { name: 'Perfil do teste' });
    await waitFor(() => {
      expect((picker as HTMLSelectElement).value).toBe('perfil-1');
    });
    expect(within(picker).getByRole('option', { name: 'Padrão (em uso no Testar)' })).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));

    const menu = await openMenu(user);
    const radios = await within(menu).findAllByRole('menuitemradio');
    expect(radios.map((radio) => radio.textContent)).toEqual([
      'PadrãoMemória automática · Java automático',
      'PC fraco (4 GB)4 GB · Java automático',
    ]);
    expect(radios[0]?.getAttribute('aria-checked')).toBe('true');
    await user.click(within(menu).getByRole('menuitemradio', { name: /PC fraco/ }));
    await waitFor(() => {
      expect(backend.callsOf('test_profile_select')[0]?.args).toMatchObject({
        profileId: 'perfil-1',
      });
    });
    // O cabeçalho relê os perfis: "Testar · PC fraco" (sem o complemento entre parênteses).
    const button = await within(header()).findByRole('button', { name: 'Testar PC fraco' });
    expect(button.textContent).toBe('Testar · PC fraco');

    await user.click(button);
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(1);
    });
    // O pedido não fixa o perfil: o backend usa o ativo (o que o menu escolheu).
    expect(backend.callsOf('test_start')[0]?.args.request).toMatchObject({ profile: null });
  });

  it('o rádio marca o perfil em uso e voltar para o Padrão tira o nome do botão; axe no menu', async () => {
    const { backend, user } = await openPack({
      profiles: makeProfilesView({
        active: 'perfil-1',
        profiles: [makeProfile('default', ''), WEAK],
      }),
    });
    await within(header()).findByRole('button', { name: 'Testar PC fraco' });
    const menu = await openMenu(user);
    const weak = await within(menu).findByRole('menuitemradio', { name: /PC fraco/ });
    expect(weak.getAttribute('aria-checked')).toBe('true');
    // O menu fica num portal, fora das regiões da página (como todo menu): axe no menu.
    expect(await axeComponent(menu)).toHaveNoViolations();
    await user.click(within(menu).getByRole('menuitemradio', { name: /Padrão/ }));
    await waitFor(() => {
      expect(backend.callsOf('test_profile_select')[0]?.args).toMatchObject({
        profileId: 'default',
      });
    });
    expect(await within(header()).findByRole('button', { name: 'Testar' })).toBeDefined();
  });
});

describe('seletor Perfil dos Ajustes do teste', () => {
  it('o Padrão não pode ser renomeado nem apagado; nome repetido é recusado sem chamar o backend', async () => {
    const { backend, user } = await openPack({
      profiles: makeProfilesView({ profiles: [makeProfile('default', ''), WEAK] }),
    });
    const dialog = await openSettings(user);
    expect(within(dialog).getByRole('button', { name: 'Renomear' }).hasAttribute('disabled')).toBe(
      true,
    );
    expect(within(dialog).getByRole('button', { name: 'Apagar' }).hasAttribute('disabled')).toBe(
      true,
    );
    expect(dialog.textContent).toContain('O perfil Padrão não pode ser renomeado nem apagado.');

    await user.click(within(dialog).getByRole('button', { name: 'Novo perfil' }));
    const name = within(dialog).getByRole('textbox', { name: 'Nome do perfil' });
    expect(document.activeElement).toBe(name);
    await user.click(within(dialog).getByRole('button', { name: 'Criar perfil' }));
    expect(within(dialog).getByRole('alert').textContent).toBe('Dê um nome ao perfil.');
    await user.type(name, 'pc fraco (4 gb)');
    expect(within(dialog).getByRole('alert').textContent).toBe(
      'Já existe um perfil com esse nome.',
    );
    await user.clear(name);
    await user.type(name, 'Padrão');
    expect(within(dialog).getByRole('alert').textContent).toBe(
      'Já existe um perfil com esse nome.',
    );
    expect(backend.callsOf('test_profile_create')).toHaveLength(0);
    // O Cancelar da linha do nome fecha só ela e devolve o foco ao seletor.
    const form = name.closest('form');
    if (!form) throw new Error('linha do nome sem formulário');
    await user.click(within(form).getByRole('button', { name: 'Cancelar' }));
    expect(within(dialog).queryByRole('textbox', { name: 'Nome do perfil' })).toBeNull();
    expect(document.activeElement).toBe(
      within(dialog).getByRole('combobox', { name: 'Perfil do teste' }),
    );
    expect(screen.getByRole('dialog', { name: 'Ajustes do teste neste computador' })).toBe(dialog);
  });

  it('renomear e apagar o perfil em uso atualizam o diálogo, o menu ▾ e o botão', async () => {
    const { backend, user } = await openPack({
      profiles: makeProfilesView({
        active: 'perfil-1',
        profiles: [makeProfile('default', ''), WEAK],
      }),
    });
    const dialog = await openSettings(user);
    const picker = within(dialog).getByRole('combobox', { name: 'Perfil do teste' });
    expect((picker as HTMLSelectElement).value).toBe('perfil-1');

    await user.click(within(dialog).getByRole('button', { name: 'Renomear' }));
    const name = within(dialog).getByRole('textbox', { name: 'Nome do perfil' });
    expect((name as HTMLInputElement).value).toBe('PC fraco (4 GB)');
    await user.clear(name);
    await user.type(name, 'Notebook');
    await user.click(within(dialog).getByRole('button', { name: 'Renomear perfil' }));
    await waitFor(() => {
      expect(backend.callsOf('test_profile_rename')[0]?.args).toMatchObject({
        profileId: 'perfil-1',
        name: 'Notebook',
      });
    });
    expect(
      await within(picker).findByRole('option', { name: 'Notebook (em uso no Testar)' }),
    ).toBeDefined();
    // O botão do cabeçalho também relê (mesma chave invalidada).
    expect(
      await within(hiddenHeader()).findByRole('button', { name: 'Testar Notebook', hidden: true }),
    ).toBeDefined();

    await user.click(within(dialog).getByRole('button', { name: 'Apagar' }));
    const confirm = await screen.findByRole('alertdialog', {
      name: 'Apagar o perfil "Notebook"?',
    });
    await user.click(within(confirm).getByRole('button', { name: 'Apagar perfil' }));
    await waitFor(() => {
      expect(backend.callsOf('test_profile_delete')).toHaveLength(1);
    });
    await waitFor(() => {
      expect((picker as HTMLSelectElement).value).toBe('default');
    });
    expect(within(picker).queryByRole('option', { name: /Notebook/ })).toBeNull();
    expect(
      await within(hiddenHeader()).findByRole('button', { name: 'Testar', hidden: true }),
    ).toBeDefined();
  });

  it('cada perfil guarda o próprio rascunho e "Salvar ajustes" grava todos os que mudaram', async () => {
    const { backend, user } = await openPack({
      profiles: makeProfilesView({ profiles: [makeProfile('default', ''), WEAK] }),
    });
    const dialog = await openSettings(user);
    const picker = within(dialog).getByRole('combobox', { name: 'Perfil do teste' });
    await user.selectOptions(
      within(dialog).getByRole('combobox', { name: 'Memória do teste' }),
      '8192',
    );
    await user.selectOptions(picker, 'perfil-1');
    const memory = within(dialog).getByRole('combobox', { name: 'Memória do teste' });
    expect((memory as HTMLSelectElement).value).toBe('4096');
    await user.selectOptions(
      within(dialog).getByRole('combobox', { name: 'Janela do jogo' }),
      'fullscreen',
    );
    // Voltar ao Padrão mostra o que foi mudado nele.
    await user.selectOptions(picker, 'default');
    expect((memory as HTMLSelectElement).value).toBe('8192');
    await user.click(within(dialog).getByRole('button', { name: 'Salvar ajustes' }));
    await waitFor(() => {
      expect(backend.callsOf('test_profiles_save')).toHaveLength(1);
    });
    expect(backend.callsOf('test_profiles_save')[0]?.args).toMatchObject({
      profiles: [
        { id: 'default', memory: { mode: 'fixed', mb: 8192 }, window: { mode: 'default' } },
        {
          id: 'perfil-1',
          name: 'PC fraco (4 GB)',
          memory: { mode: 'fixed', mb: 4096 },
          window: { mode: 'fullscreen' },
        },
      ],
    });
    expect(await screen.findByText('Ajustes do teste salvos.')).toBeDefined();
  });
});

describe('Ao abrir o jogo (CA-T11-05)', () => {
  it('pack 1.19.2: "Entrar direto no mundo de teste" aparece desabilitado com o motivo', async () => {
    const { user } = await openPack({
      pack: { minecraft: '1.19.2' },
      profiles: makeProfilesView({ minecraft: '1.19.2', quickPlay: 'tooOld' }),
    });
    const dialog = await openSettings(user);
    const world = within(dialog).getByRole('radio', { name: /Entrar direto no mundo de teste/ });
    expect(world.hasAttribute('disabled')).toBe(true);
    expect(within(dialog).getByRole('radio', { name: /menu principal/ })).toHaveProperty(
      'checked',
      true,
    );
    expect(dialog.textContent).toContain(
      'Só no Minecraft 1.20 ou mais novo. Este pack usa o 1.19.2.',
    );
    // O motivo é a descrição acessível do rádio.
    const described = world.getAttribute('aria-describedby') ?? '';
    expect(document.getElementById(described)?.textContent).toContain('1.20 ou mais novo');
  });

  it('pack 1.20.1: escolher o mundo de teste e salvar; sem mundo ainda, diz que abre no menu; axe', async () => {
    let worlds = ['Mundo de teste', 'Outro'];
    const { backend, user } = await openPack({
      handlers: { instance_worlds_list: () => worlds },
    });
    const dialog = await openSettings(user);
    const world = within(dialog).getByRole('radio', { name: /Entrar direto no mundo de teste/ });
    expect(world.hasAttribute('disabled')).toBe(false);
    await user.click(world);
    const select = await within(dialog).findByRole('combobox', { name: 'Mundo de teste' });
    expect(await within(select).findByRole('option', { name: 'Outro' })).toBeDefined();
    expect((select as HTMLSelectElement).value).toBe('');
    await user.selectOptions(select, 'Outro');
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Salvar ajustes' }));
    await waitFor(() => {
      expect(backend.callsOf('test_profiles_save')[0]?.args).toMatchObject({
        profiles: [{ id: 'default', onLaunch: { mode: 'world', world: 'Outro' } }],
      });
    });

    worlds = [];
    const again = await openSettings(user);
    await user.click(within(again).getByRole('radio', { name: /Entrar direto no mundo de teste/ }));
    expect(
      await within(again).findByText(
        'Ainda não há mundo de teste nesta instância. Crie um mundo no jogo: nos próximos testes, ele abre direto. Até lá, o jogo abre no menu principal.',
      ),
    ).toBeDefined();
  });
});

describe('avisos do perfil', () => {
  it('memória acima de 75% da RAM do computador gera aviso', async () => {
    const { user } = await openPack({
      profiles: makeProfilesView({ computerMemoryMb: 16_384 }),
    });
    const dialog = await openSettings(user);
    const memory = within(dialog).getByRole('combobox', { name: 'Memória do teste' });
    await user.selectOptions(memory, '12288');
    expect(dialog.textContent).not.toContain('Passa de 75%');
    await user.selectOptions(memory, '16384');
    expect(dialog.textContent).toContain(
      'Passa de 75% da memória deste computador (16 GB). O computador pode ficar lento ou o jogo pode fechar.',
    );
    expect(memory.getAttribute('aria-describedby')).toContain('-aviso');
  });

  it('W_PERF_FLAGS: "Remover estes argumentos" tira só os que impedem ler a memória', async () => {
    const { user } = await openPack();
    const dialog = await openSettings(user);
    const args = within(dialog).getByRole('textbox', { name: /Argumentos extras da JVM/ });
    expect(within(dialog).queryByRole('button', { name: 'Remover estes argumentos' })).toBeNull();
    await user.type(args, '-XX:+UseG1GC -XX:+PerfDisableSharedMem -XX:-UsePerfData -Dx=1');
    expect(dialog.textContent).toContain(
      '-XX:+PerfDisableSharedMem: impede o Warden de ler a memória do jogo durante o teste.',
    );
    await user.click(within(dialog).getByRole('button', { name: 'Remover estes argumentos' }));
    expect((args as HTMLTextAreaElement).value).toBe('-XX:+UseG1GC -Dx=1');
    expect(within(dialog).queryByRole('button', { name: 'Remover estes argumentos' })).toBeNull();
  });
});

describe('mods opcionais por perfil', () => {
  const CHOICES: OptionalChoice[] = [
    {
      path: 'mods/iris.pw.toml',
      name: 'Iris Shaders',
      description: 'Shaders',
      default: false,
      enabled: false,
    },
  ];

  it('fora do Padrão, a escolha fica no perfil (não na instância) e vai com "Salvar ajustes"', async () => {
    const { backend, user } = await openPack({
      profiles: makeProfilesView({ profiles: [makeProfile('default', ''), WEAK] }),
      handlers: {
        instance_optional_choices_get: () => CHOICES,
        instance_set_optional_choices: () => null,
      },
    });
    const dialog = await openSettings(user);
    await user.selectOptions(
      within(dialog).getByRole('combobox', { name: 'Perfil do teste' }),
      'perfil-1',
    );
    expect(within(dialog).getByText('Mods opcionais ligados neste perfil')).toBeDefined();
    const iris = await within(dialog).findByRole('switch', { name: 'Iris Shaders' });
    expect(iris.getAttribute('aria-checked')).toBe('false');
    await user.click(iris);
    expect(iris.getAttribute('aria-checked')).toBe('true');
    expect(backend.callsOf('instance_set_optional_choices')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: 'Salvar ajustes' }));
    await waitFor(() => {
      expect(backend.callsOf('test_profiles_save')[0]?.args).toMatchObject({
        profiles: [{ id: 'perfil-1', optionals: { 'mods/iris.pw.toml': true } }],
      });
    });
  });
});

describe('"Este teste" na tela do teste', () => {
  it('mostra o perfil do jogo em preparação, com a memória dele', async () => {
    let packId = '';
    const state = profilesHandlers(
      makeProfilesView({ active: 'perfil-1', profiles: [makeProfile('default', ''), WEAK] }),
    );
    const setup = editorBackend({
      handlers: {
        ...state.handlers,
        test_game_state: () =>
          makeGame(packId as PackRow['id'], 'preparing', { profile: 'perfil-1' }),
      },
    });
    packId = setup.row.id;
    renderApp(`${setup.url}/teste`);
    const panel = await screen.findByRole('complementary', { name: 'Este teste' });
    await waitFor(() => {
      expect(within(panel).getByText('Perfil').nextElementSibling?.textContent).toBe(
        'PC fraco (4 GB)',
      );
    });
    expect(within(panel).getByText('Memória').nextElementSibling?.textContent).toBe('4 GB');
  });
});
