/**
 * Publicar versão (SPEC T18; V-03) no app inteiro, com o backend simulado: a área "Publicação
 * para os jogadores" do Histórico, a primeira publicação, o diálogo com bloqueios, avisos,
 * troca pelo Modrinth, divergência e notas, e o resultado com o link e o passo a passo. As
 * regras da publicação (envio, linha de publicação, bytes, segredos, notas) são provadas no
 * Rust (`warden-versioning`, `commands/publish.rs`) e no E2E com o servidor git local.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { GithubStatus, PublishPlan } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend } from '../pack-editor/testing';
import { makeHistory, makeSavedVersion } from '../versioning/versioning.fixtures';
import {
  makeGithubLink,
  makeGithubStatus,
  makePublishPlan,
  makePublished,
  makePublishedVersion,
} from './github.fixtures';

type Options = NonNullable<Parameters<typeof editorBackend>[0]>;

const writeText = vi.fn<(text: string) => Promise<void>>();

beforeEach(() => {
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** Histórico com a 1.5.0 final e a 1.4.0 publicada. */
function history() {
  return makeHistory({
    lastVersion: '1.5.0',
    versions: [
      makeSavedVersion('1.5.0', { isFinal: true, message: '' }),
      makeSavedVersion('1.4.0', { isFinal: true, isPublished: true, message: '' }),
    ],
  });
}

async function openHistory(handlers: Options['handlers'] = {}) {
  const { backend, row } = editorBackend({
    pack: { version: '1.5.0' },
    handlers: { history_get: () => history(), ...handlers },
  });
  renderApp(`/packs/${row.id}/historico`);
  await screen.findByRole('heading', { level: 1, name: 'Histórico' }, { timeout: 10_000 });
  await screen.findByRole('heading', { level: 2, name: 'Versões' });
  // O `userEvent` instala a área de transferência dele; o espião vem depois.
  const user = userEvent.setup();
  vi.spyOn(navigator.clipboard, 'writeText').mockImplementation(writeText);
  return { backend, row, user };
}

function publishedStatus(overrides: Partial<GithubStatus> = {}): GithubStatus {
  return makeGithubStatus({
    link: makeGithubLink(),
    published: [makePublishedVersion('1.4.0')],
    ...overrides,
  });
}

/** O diálogo do plano já carregado (o de "Conferindo…" é outro elemento). */
async function planDialog(): Promise<HTMLElement> {
  await waitFor(() => {
    expect(screen.queryByText(/^Conferindo a versão/)).toBeNull();
    expect(
      screen.getByRole('dialog', { name: 'Publicar a versão 1.5.0 para os jogadores' }),
    ).toBeDefined();
  });
  return screen.getByRole('dialog', { name: 'Publicar a versão 1.5.0 para os jogadores' });
}

async function openPublish(user: ReturnType<typeof userEvent.setup>) {
  const button = await screen.findByRole('button', { name: 'Publicar versão 1.5.0' });
  await user.click(button);
  return button;
}

describe('Publicação para os jogadores (área do Histórico)', () => {
  it('mostra repositório, visibilidade, versão publicada e o link com Copiar link; axe', async () => {
    const { user } = await openHistory({ github_status: () => publishedStatus() });
    const panel = screen.getByRole('region', { name: 'Publicação para os jogadores' });
    await within(panel).findByText('kriticales/vale-sereno');
    expect(within(panel).getByText('Público')).toBeDefined();
    expect(panel.textContent).toContain('1.4.0, em 20/09/2026');
    const link = within(panel).getByLabelText('Link do pack');
    expect((link as HTMLInputElement).value).toBe(makeGithubLink().packLink);
    await user.click(within(panel).getByRole('button', { name: 'Copiar link' }));
    expect(writeText).toHaveBeenCalledWith(makeGithubLink().packLink);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('Como os jogadores instalam: os 4 passos com o comando do bootstrap e Copiar instruções', async () => {
    const { user } = await openHistory({ github_status: () => publishedStatus() });
    await user.click(await screen.findByRole('button', { name: 'Como os jogadores instalam' }));
    const dialog = await screen.findByRole('dialog', { name: 'Como os jogadores instalam' });
    expect(dialog.textContent).toContain(
      'Criar uma instância com Minecraft 1.20.1 e Forge 47.3.0.',
    );
    expect(dialog.textContent).toContain(
      `"$INST_JAVA" -jar packwiz-installer-bootstrap.jar ${makeGithubLink().packLink}`,
    );
    expect(dialog.textContent).toContain('Se aparecer um erro 403');
    await user.click(within(dialog).getByRole('button', { name: 'Copiar instruções' }));
    const copied = writeText.mock.calls[0]?.[0] ?? '';
    expect(copied).toContain('2. Baixar o packwiz-installer-bootstrap.jar');
    expect(copied).toContain(makeGithubLink().packLink);
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getAllByRole('button', { name: 'Fechar' })[0] ?? document.body);
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    // O foco volta para quem abriu.
    expect(document.activeElement?.textContent).toBe('Como os jogadores instalam');
  });

  it('repositório privado: "Só backup" no lugar do link e do passo a passo', async () => {
    await openHistory({
      github_status: () => publishedStatus({ link: makeGithubLink({ private: true }) }),
    });
    const panel = screen.getByRole('region', { name: 'Publicação para os jogadores' });
    await within(panel).findByText('Privado');
    expect(panel.textContent).toContain('Só backup: o link não funciona para os jogadores.');
    expect(within(panel).queryByLabelText('Link do pack')).toBeNull();
    expect(within(panel).queryByRole('button', { name: 'Como os jogadores instalam' })).toBeNull();
  });

  it('Release pendente: Tentar de novo cria só a Release e relê o pack inteiro', async () => {
    let pending = true;
    const { backend, user } = await openHistory({
      github_status: () =>
        publishedStatus({
          published: [makePublishedVersion('1.4.0', { releasePending: pending })],
        }),
      publish_retry_release: () => {
        pending = false;
        return makePublished({ version: '1.4.0' });
      },
    });
    const panel = screen.getByRole('region', { name: 'Publicação para os jogadores' });
    await within(panel).findByText(/A versão 1.4.0 foi enviada, mas a Release do GitHub/);
    const before = {
      history: backend.callsOf('history_get').length,
      row: backend.callsOf('pack_get').length,
    };
    await user.click(within(panel).getByRole('button', { name: 'Tentar de novo' }));
    expect(backend.callsOf('publish_retry_release')[0]?.args).toMatchObject({ version: '1.4.0' });
    await waitFor(() => {
      expect(within(panel).queryByText(/foi enviada, mas a Release/)).toBeNull();
    });
    expect(await screen.findByText('Release da versão 1.4.0 criada no GitHub.')).toBeDefined();
    // Dados atuais: Histórico e linha do pack relidos (Meus packs fica marcado para reler).
    await waitFor(() => {
      expect(backend.callsOf('history_get').length).toBeGreaterThan(before.history);
      expect(backend.callsOf('pack_get').length).toBeGreaterThan(before.row);
    });
  });
});

describe('Publicar versão (T18)', () => {
  it('sem token: explica e leva a Configurações, sem pedir o plano', async () => {
    const { backend, user } = await openHistory({
      github_status: () => makeGithubStatus({ tokenConfigured: false }),
    });
    await openPublish(user);
    const dialog = await screen.findByRole('dialog', { name: 'Falta o token do GitHub' });
    expect(
      within(dialog).getByRole('link', { name: 'Abrir Configurações' }).getAttribute('href'),
    ).toBe('/configuracoes');
    expect(backend.callsOf('publish_plan')).toHaveLength(0);
  });

  it('primeira publicação: cria o repositório público por padrão, publica e mostra o resultado', async () => {
    let linked = false;
    let published = false;
    const { backend, user } = await openHistory({
      github_status: () =>
        linked
          ? publishedStatus({ published: published ? [makePublishedVersion('1.5.0')] : [] })
          : makeGithubStatus(),
      github_setup: () => {
        linked = true;
        return makeGithubLink();
      },
      publish_plan: () =>
        makePublishPlan({ lastPublished: null, findings: [{ kind: 'untested' }] }),
      publish_run: () => {
        published = true;
        return makePublished();
      },
    });
    const opener = await openPublish(user);
    const setup = await screen.findByRole('dialog', { name: 'Primeira publicação deste pack' });
    expect(within(setup).getByLabelText<HTMLInputElement>('Nome do repositório').value).toBe(
      'vale-sereno',
    );
    expect(within(setup).getByRole<HTMLInputElement>('radio', { name: /Público/ }).checked).toBe(
      true,
    );
    expect(setup.textContent).toContain('Nada do seu computador vai junto.');
    expect(setup.textContent).toContain('O link não funciona para os jogadores');
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(setup).getByRole('button', { name: 'Criar repositório e continuar' }));
    expect(backend.callsOf('github_setup')[0]?.args).toMatchObject({
      setup: { kind: 'create', name: 'vale-sereno', private: false },
    });

    const plan = await planDialog();
    expect(plan.textContent).toContain('A versão 1.5.0 não foi testada.');
    expect(plan.textContent).toContain('Primeira publicação: tudo o que o pack tem.');
    expect(plan.textContent).toContain('pack.toml, index.toml, 10 arquivos do índice');
    expect(within(plan).getByRole('link', { name: 'Testar agora' }).getAttribute('href')).toContain(
      '/teste',
    );
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.type(within(plan).getByLabelText(/^Notas para os jogadores/), 'Façam backup.');
    // Com aviso, o botão diz "Publicar mesmo assim" (o aviso não bloqueia).
    await user.click(within(plan).getByRole('button', { name: 'Publicar mesmo assim' }));
    expect(backend.callsOf('publish_run')[0]?.args).toMatchObject({
      request: { version: '1.5.0', playerNotes: 'Façam backup.', replaceRemote: null },
    });

    const result = await screen.findByRole('dialog', { name: 'Versão 1.5.0 publicada' });
    expect(result.textContent).toContain('O GitHub pode levar alguns minutos');
    expect(within(result).getByLabelText<HTMLInputElement>('Link do pack').value).toBe(
      makePublished().packLink,
    );
    expect(result.textContent).toContain('Como os jogadores instalam (Prism Launcher ou MultiMC)');
    await user.click(
      within(result).getByRole('button', { name: 'Copiar texto das notas da versão' }),
    );
    expect(writeText).toHaveBeenLastCalledWith(makePublished().notes);
    await user.click(within(result).getByRole('button', { name: 'Abrir a Release no GitHub' }));
    await waitFor(() => {
      expect(backend.callsOf('plugin:opener|open_url')[0]?.args).toMatchObject({
        url: makePublished().release.htmlUrl,
      });
    });
    expect(await axePage(document.body)).toHaveNoViolations();
    // O Histórico foi relido: a versão passa a "Publicada" e o botão some.
    await waitFor(() => {
      expect(
        screen.getByRole('region', { name: 'Publicação para os jogadores', hidden: true })
          .textContent,
      ).toContain('1.5.0, em');
    });
    await user.click(within(result).getAllByRole('button', { name: 'Fechar' })[0] ?? document.body);
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(opener.isConnected).toBe(true);
  });

  it('usar um repositório que já existe, privado: avisa que é só backup', async () => {
    let linked = false;
    const { backend, user } = await openHistory({
      github_status: () =>
        linked ? publishedStatus({ link: makeGithubLink({ private: true }) }) : makeGithubStatus(),
      github_setup: () => {
        linked = true;
        return makeGithubLink({ private: true });
      },
      publish_plan: () => makePublishPlan({ link: makeGithubLink({ private: true }) }),
    });
    await openPublish(user);
    const setup = await screen.findByRole('dialog', { name: 'Primeira publicação deste pack' });
    await user.click(
      within(setup).getByRole('button', { name: 'Usar um repositório que já existe' }),
    );
    const field = within(setup).getByLabelText('Repositório (dono/nome)');
    const confirm = within(setup).getByRole('button', {
      name: 'Usar este repositório e continuar',
    });
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await user.type(field, 'kriticales/vale-sereno');
    await user.click(confirm);
    expect(backend.callsOf('github_setup')[0]?.args).toMatchObject({
      setup: { kind: 'link', repo: 'kriticales/vale-sereno' },
    });
    const plan = await planDialog();
    await within(plan).findByText(/é privado: só backup/);
    // Sem avisos, o botão é só "Publicar".
    expect(within(plan).getByRole('button', { name: 'Publicar' })).toBeDefined();
  });

  it('nome de repositório inválido não deixa criar', async () => {
    const { user } = await openHistory({ github_status: () => makeGithubStatus() });
    await openPublish(user);
    const setup = await screen.findByRole('dialog', { name: 'Primeira publicação deste pack' });
    const name = within(setup).getByLabelText('Nome do repositório');
    await user.clear(name);
    await user.type(name, 'nome com espaço');
    expect(setup.textContent).toContain('Use só letras sem acento');
    expect(
      within(setup).getByRole<HTMLButtonElement>('button', {
        name: 'Criar repositório e continuar',
      }).disabled,
    ).toBe(true);
  });

  it('CA-T18-06: algo com cara de token bloqueia apontando arquivo e linha', async () => {
    const { backend, user } = await openHistory({
      github_status: () => publishedStatus(),
      publish_plan: (): PublishPlan =>
        makePublishPlan({
          findings: [{ kind: 'secretFound', path: 'config/x.toml', line: 3 }, { kind: 'untested' }],
          blocked: true,
        }),
    });
    await openPublish(user);
    const plan = await planDialog();
    await within(plan).findByText(
      'Encontramos algo que parece uma chave em config/x.toml, linha 3. Remova antes de publicar.',
    );
    const publish = within(plan).getByRole('button', { name: 'Publicar mesmo assim' });
    expect((publish as HTMLButtonElement).disabled).toBe(true);
    await user.click(publish);
    expect(backend.callsOf('publish_run')).toHaveLength(0);
  });

  it('CA-T18-08: os 2 mods só da CurseForge, Trocar pelo Modrinth só no que existe lá', async () => {
    const { backend, user } = await openHistory({
      github_status: () => publishedStatus(),
      publish_plan: () =>
        makePublishPlan({
          findings: [
            {
              kind: 'curseforgeOnly',
              mods: [
                {
                  path: 'mods/jei.pw.toml',
                  name: 'Just Enough Items',
                  swap: {
                    projectId: 'u6dRKJwZ',
                    versionId: 'vJ',
                    versionNumber: '15.20.0.106',
                    fileName: 'jei.jar',
                    sameFile: true,
                    side: 'both',
                  },
                },
                { path: 'mods/xaero.pw.toml', name: "Xaero's Minimap", swap: null },
              ],
            },
          ],
        }),
      add_apply: () => ({ added: [], skipped: [], removed: ['mods/jei.pw.toml'] }),
    });
    await openPublish(user);
    const plan = await planDialog();
    await within(plan).findByText(/2 mods só existem na CurseForge/);
    expect(plan.textContent).toContain('Just Enough Items');
    expect(plan.textContent).toContain("Xaero's Minimap");
    expect(within(plan).getAllByRole('button', { name: /pelo Modrinth/ })).toHaveLength(1);
    expect(plan.textContent).toContain('depois é preciso salvar uma versão nova');
    const before = backend.callsOf('history_get').length;
    await user.click(
      within(plan).getByRole('button', { name: 'Trocar Just Enough Items pelo Modrinth' }),
    );
    expect(backend.callsOf('add_apply')[0]?.args).toMatchObject({
      request: {
        items: [
          {
            source: 'modrinth',
            projectId: 'u6dRKJwZ',
            versionId: 'vJ',
            side: 'both',
            replaces: 'mods/jei.pw.toml',
          },
        ],
      },
    });
    await waitFor(() => {
      expect(within(plan).queryByRole('button', { name: /pelo Modrinth/ })).toBeNull();
    });
    // A troca muda o pack: Histórico (alterações não salvas), linha e lista relidos.
    await waitFor(() => {
      expect(backend.callsOf('history_get').length).toBeGreaterThan(before);
    });
    // O aviso não bloqueia.
    expect(
      within(plan).getByRole<HTMLButtonElement>('button', { name: 'Publicar mesmo assim' })
        .disabled,
    ).toBe(false);
  });

  it('GitHub divergente: só publica com a dupla confirmação, para o commit visto', async () => {
    const remoteMain = 'b'.repeat(40);
    const { backend, user } = await openHistory({
      github_status: () => publishedStatus(),
      publish_plan: () => makePublishPlan({ remote: { remoteMain, inSync: false } }),
      publish_run: () => makePublished(),
    });
    await openPublish(user);
    const plan = await planDialog();
    await within(plan).findByText('O GitHub tem mudanças que não estão aqui');
    expect(within(plan).getByRole<HTMLButtonElement>('button', { name: 'Publicar' }).disabled).toBe(
      true,
    );
    await user.click(
      within(plan).getByRole('button', { name: 'Substituir o GitHub pelo que está aqui' }),
    );
    const confirm = await screen.findByRole('alertdialog', {
      name: 'Substituir o GitHub pelo que está aqui?',
    });
    const go = within(confirm).getByRole('button', { name: 'Substituir e publicar' });
    expect((go as HTMLButtonElement).disabled).toBe(true);
    await user.type(within(confirm).getByRole('textbox'), 'Vale Sereno');
    await user.click(go);
    await waitFor(() => {
      expect(backend.callsOf('publish_run')[0]?.args).toMatchObject({
        request: { replaceRemote: { expectedMain: remoteMain } },
      });
    });
    await screen.findByRole('dialog', { name: 'Versão 1.5.0 publicada' });
  });

  it('Release pendente depois de enviar: Tentar de novo cria só a Release', async () => {
    const { backend, user } = await openHistory({
      github_status: () => publishedStatus(),
      publish_plan: () => makePublishPlan(),
      publish_run: () =>
        ipcError(
          makeAppError(
            { domain: 'versioning', code: 'RELEASE_PENDING' },
            { params: { version: '1.5.0' }, retryable: true },
          ),
        ),
      publish_retry_release: () => makePublished(),
    });
    await openPublish(user);
    const plan = await planDialog();
    await user.click(await within(plan).findByRole('button', { name: 'Publicar' }));
    await within(plan).findByText(/foi enviada aos jogadores, mas a página da versão no GitHub/);
    await user.click(within(plan).getByRole('button', { name: 'Tentar de novo' }));
    expect(backend.callsOf('publish_retry_release')[0]?.args).toMatchObject({ version: '1.5.0' });
    await screen.findByRole('dialog', { name: 'Versão 1.5.0 publicada' });
    expect(backend.callsOf('publish_run')).toHaveLength(1);
  });
});
