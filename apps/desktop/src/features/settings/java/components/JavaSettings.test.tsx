import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type {
  InstalledRuntime,
  JavaChoiceReason,
  JavaDecision,
  JavaOverview,
  JavaVersion,
  PackJavaUse,
  UpdateReport,
} from '../../../../lib/ipc/bindings';
import { axeComponent } from '../../../../test/axe';
import { deferred, ipcError, mockBackend } from '../../../../test/backend';
import { makeAppError } from '../../../../test/factories';
import { renderWithProviders } from '../../../../test/render';
import { JavaSettings } from './JavaSettings';

function version(major: number, security: number, patch = 0): JavaVersion {
  return { major, minor: 0, security, patch, build: 1 };
}

function runtime(
  id: string,
  v: JavaVersion,
  extra: Partial<InstalledRuntime> = {},
): InstalledRuntime {
  return {
    id,
    source: 'temurin',
    version: v,
    vendor: 'Eclipse Adoptium',
    releaseName: id,
    arch: 'x64',
    home: `C:/dados/shared/runtimes/${id}`,
    launcher: `C:/dados/shared/runtimes/${id}/bin/javaw.exe`,
    java: `C:/dados/shared/runtimes/${id}/bin/java.exe`,
    installedAtMs: 0,
    updateCap: null,
    mojangComponent: null,
    archiveSha256: null,
    supersededBy: null,
    ...extra,
  };
}

function decision(
  reason: JavaChoiceReason,
  major: number,
  rangeLabel: string | null,
): JavaDecision {
  return {
    requirement: { major, maxUpdate: null },
    reason,
    ruleId: null,
    rangeLabel,
    explanation: 'texto da tabela',
    newestMajor: 25,
    versionJsonMajor: null,
  };
}

function packUse(
  name: string,
  minecraft: string,
  automatic: JavaDecision,
  installed: InstalledRuntime | null,
): PackJavaUse {
  return {
    packId: name,
    name,
    minecraft,
    choice: {
      major: automatic.requirement.major,
      maxUpdate: null,
      reason: automatic.reason,
      runtime: installed,
      automatic,
      userChoiceUnavailable: false,
    },
  };
}

const java17Old = runtime('temurin-17-jdk-17.0.15-x64', version(17, 15));
const java17New = runtime('temurin-17-jdk-17.0.20.1+1-x64', version(17, 20, 1));
const java8 = runtime('temurin-8-jdk8u504-b01-x64', version(8, 504));
const range17 = decision('NEWEST_PROVEN_FOR_RANGE', 17, '1.17 a 1.20.4');
const legacy8 = decision('FORGE_LEGACY_JAVA8', 8, 'até o 1.12.2');

function overview(java17: InstalledRuntime, extra: Partial<JavaOverview> = {}): JavaOverview {
  return {
    runtimes: [
      {
        runtime: java17,
        packs: [packUse('Vale Sereno', '1.20.1', range17, java17)],
        inGame: false,
      },
      {
        runtime: java8,
        packs: [packUse('Técnico Clássico', '1.7.10', legacy8, java8)],
        inGame: false,
      },
    ],
    packsToDownload: [
      packUse('Leve e Bonito', '26.3', decision('NEWEST_AVAILABLE', 25, '26.x'), null),
    ],
    unresolved: [],
    brokenCount: 0,
    ...extra,
  };
}

function rowOf(name: string): HTMLElement {
  const header = screen.getByRole('rowheader', { name: new RegExp(name) });
  const row = header.closest('tr');
  if (!row) throw new Error(`linha ${name} sem <tr>`);
  return row;
}

describe('Configurações → Teste → Java (T21)', () => {
  it('CA-T21-04: cada Java com os packs que o usam e o motivo; passa no axe', async () => {
    mockBackend({ java_runtimes_list: () => overview(java17Old) });
    const { container } = renderWithProviders(<JavaSettings />);
    expect(screen.getByRole('group', { name: 'Java' })).toBeDefined();
    const table = await screen.findByRole('table', {
      name: 'Javas instalados pelo Warden e os packs que usam cada um',
    });
    const headers = within(table)
      .getAllByRole('rowheader')
      .map((cell) => cell.textContent);
    // Mais novo primeiro; o 25 ainda será baixado.
    expect(headers).toEqual([
      'Java 25Ainda não baixado',
      'Java 1717.0.15 · Temurin',
      'Java 88u504 · Temurin',
    ]);
    const legacy = rowOf('Java 8');
    expect(within(legacy).getByText('Técnico Clássico (1.7.10)')).toBeDefined();
    // O motivo do Forge 1.7.10 diz que essa versão só abre no Java 8.
    expect(
      within(legacy).getByText(
        'O Forge 1.7.10 e 1.12.2 só abre no Java 8. A partir do Java 9 ele trava ao iniciar.',
      ),
    ).toBeDefined();
    expect(
      within(rowOf('Java 17')).getByText(
        'O Minecraft 1.17 a 1.20.4 foi feito para o Java 17. Um Java mais novo ainda não foi provado com essas versões.',
      ),
    ).toBeDefined();
    const toDownload = rowOf('Java 25');
    expect(within(toDownload).getByText('Leve e Bonito (26.3)')).toBeDefined();
    expect(within(toDownload).getByText('Será baixado no próximo teste')).toBeDefined();
    expect(within(toDownload).getByText('O Java mais novo que o Warden usa.')).toBeDefined();
    expect(within(toDownload).queryByRole('button')).toBeNull();
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('CA-T21-04: "Procurar atualizações do Java" instala a nova e a antiga sai da tabela', async () => {
    const user = userEvent.setup();
    let current = overview(java17Old);
    const report: UpdateReport = {
      updated: [{ from: java17Old.id, to: java17New, oldRemoved: true }],
      upToDate: [java8.id],
      failed: [],
      removed: [java17Old.id],
    };
    const backend = mockBackend({
      java_runtimes_list: () => current,
      java_runtimes_check_updates: () => {
        current = overview(java17New);
        return report;
      },
    });
    renderWithProviders(<JavaSettings />);
    expect(await screen.findByText('17.0.15 · Temurin')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Procurar atualizações do Java' }));
    expect(await screen.findByText('1 Java atualizado.')).toBeDefined();
    expect(await screen.findByText('17.0.20.1 · Temurin')).toBeDefined();
    expect(screen.queryByText('17.0.15 · Temurin')).toBeNull();
    expect(backend.callsOf('java_runtimes_check_updates')).toHaveLength(1);
  });

  it('com um jogo aberto, a antiga fica marcada como substituída e não pode ser removida', async () => {
    const user = userEvent.setup();
    const superseded = { ...java17Old, supersededBy: java17New.id };
    mockBackend({
      java_runtimes_list: () =>
        overview(java17New, {
          runtimes: [
            ...overview(java17New).runtimes,
            { runtime: superseded, packs: [], inGame: true },
          ],
        }),
      java_runtimes_check_updates: (): UpdateReport => ({
        updated: [{ from: java17Old.id, to: java17New, oldRemoved: false }],
        upToDate: [],
        failed: [],
        removed: [],
      }),
    });
    renderWithProviders(<JavaSettings />);
    await screen.findByRole('table');
    const old = rowOf('17.0.15');
    expect(
      within(old).getByText(
        'Substituído pela versão 17.0.20.1. Será removido quando nenhum jogo o usar.',
      ),
    ).toBeDefined();
    expect(within(old).getByText('Em uso por um jogo aberto')).toBeDefined();
    const remove = within(old).getByRole('button', { name: 'Remover o Java 17.0.15' });
    expect((remove as HTMLButtonElement).disabled).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Procurar atualizações do Java' }));
    expect(await screen.findByText('1 Java atualizado.')).toBeDefined();
    expect(
      screen.getByText(
        'O Java antigo continua instalado até o jogo aberto fechar, e depois é removido.',
      ),
    ).toBeDefined();
  });

  it('remover um Java pede confirmação e diz quem vai baixar de novo', async () => {
    const user = userEvent.setup();
    const after = overview(java17New, { runtimes: overview(java17New).runtimes.slice(1) });
    const backend = mockBackend({
      java_runtimes_list: () => overview(java17New),
      java_runtime_remove: () => after,
    });
    renderWithProviders(<JavaSettings />);
    await user.click(await screen.findByRole('button', { name: 'Remover o Java 17.0.20.1' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover o Java 17.0.20.1?' });
    expect(
      within(dialog).getByText(
        'O pack Vale Sereno usa este Java. Ele será baixado de novo no próximo teste desse pack.',
      ),
    ).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Remover Java' }));
    await waitFor(() => {
      expect(screen.queryByText('17.0.20.1 · Temurin')).toBeNull();
    });
    expect(backend.callsOf('java_runtime_remove')[0]?.args).toEqual({ id: java17New.id });
    expect(screen.getByText('1 Java removido.')).toBeDefined();
  });

  it('"Remover Javas sem uso" confirma com a contagem e atualiza a tabela', async () => {
    const user = userEvent.setup();
    const unused = runtime('temurin-21-jdk-21.0.12.1+1-x64', version(21, 12, 1));
    const before = overview(java17New, {
      runtimes: [...overview(java17New).runtimes, { runtime: unused, packs: [], inGame: false }],
      brokenCount: 1,
    });
    mockBackend({
      java_runtimes_list: () => before,
      java_runtimes_remove_unused: () => overview(java17New),
    });
    renderWithProviders(<JavaSettings />);
    expect(
      within(await screen.findByRole('table')).getByText(
        'Sem uso. "Remover Javas sem uso" apaga este Java.',
      ),
    ).toBeDefined();
    expect(
      screen.getByText('Há 1 pasta de Java incompleta. "Remover Javas sem uso" também a apaga.'),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Remover Javas sem uso' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover os Javas sem uso?' });
    expect(within(dialog).getByText('1 Java que nenhum pack usa será apagado.')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Remover Javas sem uso' }));
    expect(await screen.findByText('1 Java removido.')).toBeDefined();
    expect(screen.queryByText('21.0.12.1 · Temurin')).toBeNull();
  });

  it('sem nada para remover, avisa sem abrir confirmação', async () => {
    const user = userEvent.setup();
    const backend = mockBackend({ java_runtimes_list: () => overview(java17New) });
    renderWithProviders(<JavaSettings />);
    await screen.findByRole('table');
    await user.click(screen.getByRole('button', { name: 'Remover Javas sem uso' }));
    expect(await screen.findByText('Nenhum Java sem uso para remover.')).toBeDefined();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(backend.callsOf('java_runtimes_remove_unused')).toHaveLength(0);
  });

  it('vazio: nenhum Java baixado ainda', async () => {
    mockBackend({
      java_runtimes_list: (): JavaOverview => ({
        runtimes: [],
        packsToDownload: [],
        unresolved: [],
        brokenCount: 0,
      }),
    });
    renderWithProviders(<JavaSettings />);
    expect(
      await screen.findByText(
        'Nenhum Java baixado ainda. O Java certo é baixado no primeiro teste de cada pack.',
      ),
    ).toBeDefined();
    expect(screen.queryByRole('table')).toBeNull();
  });

  it('packs sem decisão aparecem num aviso', async () => {
    mockBackend({
      java_runtimes_list: () =>
        overview(java17New, {
          unresolved: [
            { packId: 'a', name: 'Snapshot A', minecraft: '24w14a' },
            { packId: 'b', name: 'Snapshot B', minecraft: '25w02a' },
          ],
        }),
    });
    renderWithProviders(<JavaSettings />);
    expect(
      await screen.findByText(
        'O Warden não sabe qual Java usar com Snapshot A e Snapshot B. Escolha o Java em Ajustes do teste de cada um.',
      ),
    ).toBeDefined();
  });

  it('carregando, erro e "Tentar de novo"', async () => {
    const user = userEvent.setup();
    const first = deferred<never>();
    let calls = 0;
    mockBackend({
      java_runtimes_list: () => {
        calls += 1;
        return calls === 1 ? first.promise : overview(java17New);
      },
    });
    renderWithProviders(<JavaSettings />);
    expect(screen.getByRole('status').textContent).toBe('Lendo os Javas instalados…');
    first.fail(makeAppError({ domain: 'core', code: 'IO' }, { params: { path: 'C:/x' } }));
    await user.click(await screen.findByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByRole('table')).toBeDefined();
  });

  it('erro ao procurar atualizações mostra a frase do domínio java', async () => {
    const user = userEvent.setup();
    mockBackend({
      java_runtimes_list: () => overview(java17New),
      java_runtimes_check_updates: () =>
        ipcError(
          makeAppError(
            { domain: 'java', code: 'SOURCE_UNAVAILABLE' },
            { params: { source: 'Adoptium' } },
          ),
        ),
    });
    renderWithProviders(<JavaSettings />);
    await user.click(await screen.findByRole('button', { name: 'Procurar atualizações do Java' }));
    expect(
      await screen.findByText(
        'A fonte de downloads do Java (Adoptium) está fora do ar ou recusou o pedido. Tente de novo em alguns minutos.',
      ),
    ).toBeDefined();
  });

  it('atualização com a fonte fora do ar avisa quantos não foram conferidos', async () => {
    const user = userEvent.setup();
    mockBackend({
      java_runtimes_list: () => overview(java17New),
      java_runtimes_check_updates: (): UpdateReport => ({
        updated: [],
        upToDate: [java8.id],
        failed: [java17New.id],
        removed: [],
      }),
    });
    renderWithProviders(<JavaSettings />);
    await user.click(await screen.findByRole('button', { name: 'Procurar atualizações do Java' }));
    expect(
      await screen.findByText(
        'Não foi possível conferir 1 Java agora. A fonte de downloads não respondeu; tente de novo mais tarde.',
      ),
    ).toBeDefined();
  });

  it('tudo atualizado', async () => {
    const user = userEvent.setup();
    mockBackend({
      java_runtimes_list: () => overview(java17New),
      java_runtimes_check_updates: (): UpdateReport => ({
        updated: [],
        upToDate: [java17New.id, java8.id],
        failed: [],
        removed: [],
      }),
    });
    renderWithProviders(<JavaSettings headingLevel={4} />);
    expect(screen.getByRole('heading', { level: 4, name: 'Java' })).toBeDefined();
    await user.click(await screen.findByRole('button', { name: 'Procurar atualizações do Java' }));
    expect(
      await screen.findByText('Todos os Javas já estão na atualização mais recente.'),
    ).toBeDefined();
  });
});
