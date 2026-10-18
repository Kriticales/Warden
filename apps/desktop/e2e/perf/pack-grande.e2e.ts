/**
 * Desempenho com o pack grande no app real (A-04; SPEC §8): 300 mods, 150 resource packs, 50
 * shaders (500 itens na lista de Mods) e 500 configs reais, gerados por `comum.ts`.
 *
 * - Abrir o pack (com o cache): de clicar no nome em Meus packs até a lista de Mods na tela,
 *   com a página recarregada a cada medida (nada da abertura anterior na memória da
 *   interface); meta < 1 s.
 * - CA-T06-05: rolar a lista de 500 itens, medindo o ritmo de quadros contra o do WebView parado
 *   (60 por segundo na máquina de referência, que a CI não tem). No Windows da CI (sem GPU) o
 *   teste confere pelo menos 30 por segundo; no Linux (WebKitGTK no Xvfb, tudo pintado na CPU),
 *   só a rolagem e a lista. Os números vão para o log.
 * - Problemas: "Verificar agora" (a passagem rápida da D-01: pack, APIs pelo cache, regras e
 *   relatório), pelo comando que o botão chama; meta < 3 s.
 * - Configs: o índice da busca em todas as configs pronto ao abrir a seção (< 2 s) e cada busca
 *   (< 200 ms, pelo mesmo comando que a tela usa).
 * - Exportar (E-01): o .zip do pack, com o `packwiz refresh` em staging; a SPEC não dá meta
 *   própria, vale a de "Atualizar índice" (< 5 s), o passo mais pesado dela.
 *
 * Cada medida roda várias vezes e o teste confere a mediana, com a folga do build de debug na CI
 * (`DEBUG_MARGIN`, em `comum.ts`); mediana e pior caso saem no log (`A-04 E2E | …`), contra a
 * meta exata, para o `docs/DESEMPENHO.md`. As medidas de tempo são feitas dentro do WebView
 * (`performance.now()`), sem o atraso do WebDriver.
 */
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { seriousViolations } from '../axe';
import { openOrImport } from '../open-or-import';
import { dismissToasts } from '../toasts';
import { BIG_PACK, e2eLimit, report, summary, writePack } from './comum';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PICK_FILE = env('WARDEN_E2E_PICK_FOLDER');
const PACK = join(DATA_ROOT, 'fora', 'pack-grande');
const OUT = join(DATA_ROOT, 'exportados-pack-grande');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

/** Quantas vezes cada medida roda. */
const RUNS = 5;
/** Itens na lista de Mods. */
const ITEMS = BIG_PACK.mods + BIG_PACK.resourcePacks + BIG_PACK.shaders;

function sectionLink(name: string) {
  return $('nav[aria-label="Seções do pack"]').$(`a*=${name}`);
}

/** Quando a medida termina (avaliado no WebView, quadro a quadro). */
type Done = 'mods' | 'exported';

/**
 * Teto de cada medida dentro do WebView, só para o teste não ficar preso: a meta é conferida
 * pela mediana, não por este teto. A exportação ganha um teto maior porque a primeira cópia do
 * pack grande para o staging, no Windows da CI (sem GPU, com o antivírus examinando cada arquivo
 * novo e o build de debug), já passou de 25 s sem o app ter travado (`docs/DESEMPENHO.md`, §
 * "Exportar `.zip` no Windows da CI"). O `script` do WebDriver (30 s por padrão) sobe junto, em
 * `before`.
 */
const GUARD_MS: Record<Done, number> = { mods: 25_000, exported: 60_000 };
const SCRIPT_TIMEOUT_MS = 90_000;

/**
 * Clica no elemento (pelo seletor e pelo texto exato) dentro do WebView e espera, quadro a
 * quadro, a condição `done`. Devolve os milissegundos entre o clique e o quadro em que ela
 * valeu. As condições ficam no próprio script: a CSP do app não deixa montar código na página.
 */
async function clickAndTime(selector: string, text: string, done: Done): Promise<number> {
  return browser.execute(
    (css: string, label: string, condition: Done, guardMs: number) =>
      new Promise<number>((resolve, reject) => {
        const buttonWith = (name: string) =>
          Array.from(document.querySelectorAll<HTMLElement>(css)).find(
            (element) => element.textContent.trim() === name,
          );
        const target = buttonWith(label);
        if (!target) {
          reject(new Error(`sem ${css} "${label}"`));
          return;
        }
        const main = () => document.querySelector('main');
        const isDone = (): boolean => {
          switch (condition) {
            // A lista de Mods na tela: o título e a primeira linha de item.
            case 'mods':
              return (
                main()?.querySelector('h1')?.textContent === 'Mods' &&
                document.querySelector('tr.modrow') !== null
              );
            // O diálogo do resultado ou o erro.
            case 'exported':
              return (
                Array.from(document.querySelectorAll('[role="dialog"]')).some((dialog) =>
                  dialog.textContent.includes('Pack exportado'),
                ) || main()?.querySelector('.alert--danger') != null
              );
          }
        };
        const start = performance.now();
        target.click();
        const check = () => {
          if (isDone()) resolve(performance.now() - start);
          else if (performance.now() - start > guardMs)
            reject(new Error(`não terminou: ${condition}`));
          else requestAnimationFrame(check);
        };
        requestAnimationFrame(check);
      }),
    selector,
    text,
    done,
    GUARD_MS[done],
  );
}

/** Chama um comando do app de dentro do WebView e mede o tempo da resposta. */
async function timedCommand(command: string, args: Record<string, unknown>): Promise<number> {
  return browser.execute(
    async (name: string, payload: Record<string, unknown>) => {
      const { invoke } = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      const start = performance.now();
      await invoke(name, payload);
      return performance.now() - start;
    },
    command,
    args,
  );
}

/** O ID do pack aberto, pelo endereço da página. */
async function openPackId(): Promise<string> {
  const href = await browser.execute(() => window.location.href);
  const id = /\/packs\/([0-9A-Z]{26})/.exec(href)?.[1];
  if (!id) throw new Error(`sem pack no endereço ${href}`);
  return id;
}

describe('Desempenho com o pack grande (A-04; SPEC §8)', () => {
  before(async () => {
    rmSync(PACK, { recursive: true, force: true });
    rmSync(OUT, { recursive: true, force: true });
    mkdirSync(OUT, { recursive: true });
    writePack(PACK, BIG_PACK);
    writeFileSync(PICK_FILE, '');
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    await browser.reloadSession();
    await browser.setWindowSize(1366, 860);
    await browser.setTimeout({ script: SCRIPT_TIMEOUT_MS });
  });

  after(() => {
    try {
      const registry = JSON.parse(readFileSync(REGISTRY_FILE, 'utf8')) as {
        packs?: { id: string }[];
      };
      for (const pack of registry.packs ?? []) {
        rmSync(join(DATA_ROOT, 'data', 'instances', pack.id), { recursive: true, force: true });
      }
    } catch {
      // Sem registro: nada foi aberto.
    }
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACK, { recursive: true, force: true });
    rmSync(OUT, { recursive: true, force: true });
  });

  it('abre o pack grande pelo diálogo Abrir pack, com os 500 itens', async () => {
    await $('h2=Você ainda não tem packs').waitForDisplayed({ timeout: 30_000 });
    writeFileSync(PICK_FILE, PACK);
    await openOrImport();
    await $('[role="dialog"]').$('h2=Abrir pack').waitForDisplayed();
    const open = $('button=Abrir pack');
    const notNow = $('button=Agora não');
    await browser.waitUntil(async () => (await open.isExisting()) || (await notNow.isExisting()), {
      timeout: 60_000,
      timeoutMsg: 'Abrir pack não ficou pronto',
    });
    const started = Date.now();
    if (await notNow.isExisting()) await notNow.click();
    else await open.click();
    await $('h1=Mods').waitForDisplayed({ timeout: 60_000 });
    await $('tr.modrow').waitForExist({ timeout: 60_000 });
    process.stdout.write(
      `A-04 E2E | importar o pack grande (git do zero) | ${String(Date.now() - started)} ms\n`,
    );
    await expect($('main')).toHaveText(expect.stringContaining('Item de Bancada mods 000'));
  });

  it('abrir o pack de 300 mods com o cache (mediana de 5; meta de 1 s, com a folga do debug)', async () => {
    const times: number[] = [];
    for (let run = 0; run < RUNS; run += 1) {
      // De volta a Meus packs e a página recarregada: nada da abertura anterior fica na
      // memória da interface; o app (e o cache de metadados) continua o mesmo, como no uso.
      await $('header.packhead').$('a*=Meus packs').click();
      await $('h1=Meus packs').waitForDisplayed();
      await browser.refresh();
      await $('button.packrow__title=Pack Grande').waitForDisplayed({ timeout: 30_000 });
      times.push(await clickAndTime('button.packrow__title', 'Pack Grande', 'mods'));
    }
    const median = report('abrir o pack de 300 mods (com cache) até a lista', times, 1000);
    expect(median).toBeLessThan(e2eLimit(1000));
  });

  it('CA-T06-05: rolar a lista de 500 itens mantém o ritmo de quadros', async () => {
    await $('tr.modrow').waitForExist({ timeout: 30_000 });
    await expect($('main')).toHaveText(expect.stringContaining(`${String(ITEMS)} itens`));
    const frames = await browser.execute(
      () =>
        new Promise<{ idle: number[]; scrolling: number[]; scrolled: number; last: string }>(
          (resolve, reject) => {
            const main = document.getElementById('conteudo');
            if (!main) {
              reject(new Error('sem a área de rolagem'));
              return;
            }
            main.scrollTop = 0;
            const idle: number[] = [];
            const scrolling: number[] = [];
            let previous = 0;
            const started = performance.now();
            // 60 quadros parados (o ritmo do WebView nesta máquina), depois até 240 rolando
            // 60 px por quadro (3.600 px/s, uma rolagem rápida de roda do mouse), com teto de
            // 20 s para caber no tempo do WebDriver numa máquina lenta.
            const step = (now: number) => {
              if (previous > 0) {
                if (idle.length < 60) idle.push(now - previous);
                else scrolling.push(now - previous);
              }
              previous = now;
              if (idle.length >= 60) main.scrollTop += 60;
              if (scrolling.length < 240 && now - started < 20_000) {
                requestAnimationFrame(step);
              } else {
                const rows = document.querySelectorAll('tr.modrow');
                resolve({
                  idle,
                  scrolling,
                  scrolled: main.scrollTop,
                  last: rows[rows.length - 1]?.textContent ?? '',
                });
              }
            };
            requestAnimationFrame(step);
          },
        ),
    );
    const idle = summary(frames.idle);
    const scrolling = summary(frames.scrolling);
    // Quadros que passaram de dois intervalos da tela (mais de um quadro perdido).
    const slow = frames.scrolling.filter((delta) => delta > idle.median * 2.5).length;
    process.stdout.write(
      `A-04 E2E | CA-T06-05 (${process.platform}): quadro parado ${idle.median.toFixed(1)} ms; ` +
        `rolando mediana ${scrolling.median.toFixed(1)} ms, pior ${scrolling.worst.toFixed(1)} ms, ` +
        `${String(slow)} de ${String(frames.scrolling.length)} quadros acima de 2,5 vezes\n`,
    );
    // A rolagem andou e a lista virtualizada acompanhou (não ficou em branco).
    expect(frames.scrolled).toBeGreaterThan(3_000);
    expect(frames.last).toContain('Item de Bancada');
    // O critério da SPEC (60 por segundo) vale para a máquina de referência (Windows 11, WebView2
    // com GPU), que a CI não tem: ali a medida vai para o log e para o docs/DESEMPENHO.md, e o
    // dono confere na máquina de referência. Sem GPU, o WebKitGTK no Xvfb da CI do Linux pinta
    // tudo na CPU: cada quadro em que a lista troca de linha repinta a tela inteira (100 a 300
    // ms), então ali só a rolagem e a lista são conferidas. No Windows da CI (WebView2 sem GPU),
    // os quadros vêm em múltiplos do intervalo da tela (15,6 ms): o teste confere que a mediana
    // não passa de dois intervalos (30 por segundo) e que no máximo 1 em 10 quadros passa de
    // 2,5 intervalos (mais de um quadro perdido seguido).
    if (process.platform === 'win32') {
      expect(scrolling.median).toBeLessThanOrEqual(idle.median * 2 + 1);
      expect(slow).toBeLessThanOrEqual(frames.scrolling.length / 10);
    }
  });

  it('Problemas: passagem rápida (mediana de 5; meta de 3 s, com a folga do debug)', async () => {
    await sectionLink('Problemas').click();
    await $('h1=Problemas').waitForDisplayed();
    // A primeira verificação é automática (pack nunca verificado): espera ela terminar.
    await $('main').$('button=Verificar agora').waitForDisplayed({ timeout: 60_000 });
    const packId = await openPackId();
    // "Verificar agora" chama este comando: pack, APIs (cache), regras e relatório gravado.
    const times: number[] = [];
    for (let run = 0; run < RUNS; run += 1) {
      times.push(await timedCommand('diagnostics_run', { packId }));
    }
    const median = report('Problemas: Verificar agora (passagem rápida)', times, 3000);
    expect(median).toBeLessThan(e2eLimit(3000));
    expect(await seriousViolations()).toEqual([]);
  });

  it('Configs: índice de 500 configs (meta de 2 s) e cada busca (meta de 200 ms), com a folga do debug', async () => {
    const packId = await openPackId();
    const ready = await browser.execute(
      (id: string) =>
        new Promise<number>((resolve, reject) => {
          const link = Array.from(
            document.querySelectorAll<HTMLAnchorElement>('nav[aria-label="Seções do pack"] a'),
          ).find((a) => a.textContent.includes('Configs'));
          if (!link) {
            reject(new Error('sem o link Configs'));
            return;
          }
          const { invoke } = (
            window as unknown as {
              __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
            }
          ).__TAURI_INTERNALS__;
          const start = performance.now();
          link.click();
          // O índice nasce quando a seção abre; a busca vazia diz a situação dele.
          const poll = () => {
            void invoke('config_search', {
              packId: id,
              origin: 'pack',
              query: '',
              modId: null,
              onlyChanged: false,
            })
              .then((answer) => {
                const status = (answer as { status: { ready: boolean } }).status;
                if (status.ready) resolve(performance.now() - start);
                else if (performance.now() - start > 25_000)
                  reject(new Error('índice não ficou pronto'));
                else setTimeout(poll, 25);
              })
              .catch(reject);
          };
          poll();
        }),
      packId,
    );
    process.stdout.write(
      `A-04 E2E | Configs: índice de 500 configs pronto | ${ready.toFixed(0)} ms\n`,
    );
    expect(ready).toBeLessThan(e2eLimit(2000));

    const times: number[] = [];
    for (const query of ['spawn', 'enable', 'render distance', 'max', 'a']) {
      for (let run = 0; run < 3; run += 1) {
        times.push(
          await timedCommand('config_search', {
            packId,
            origin: 'pack',
            query,
            modId: null,
            onlyChanged: false,
          }),
        );
      }
    }
    const median = report('Configs: cada busca em todas as configs (comando)', times, 200);
    expect(median).toBeLessThan(e2eLimit(200));

    // Na tela, a busca acha as chaves do corpus agrupadas por arquivo.
    const box = $('input[aria-label="Buscar em todas as configs"]');
    await box.click();
    await box.addValue('spawn');
    await $('.cfg-search .hits__group').waitForDisplayed({ timeout: 10_000 });
  });

  it('Exportar: o .zip do pack grande (mediana de 3; referência de 5 s, com o dobro de folga)', async function () {
    // Três exportações com o teto de 60 s cada (GUARD_MS), mais o diálogo do resultado.
    this.timeout(200_000);
    await browser.keys(['Escape']);
    await sectionLink('Exportar').click();
    await $('h1=Exportar').waitForDisplayed();
    await $('h2=3. O que vai no pack').waitForDisplayed({ timeout: 60_000 });
    await $('label*=Arquivo .zip do pack packwiz').click();
    const times: number[] = [];
    for (let run = 0; run < 3; run += 1) {
      writeFileSync(PICK_FILE, join(OUT, `pack-${String(run)}.zip`));
      await dismissToasts();
      times.push(await clickAndTime('main button', 'Exportar…', 'exported'));
      await expect($('main .alert--danger')).not.toBeExisting();
      await $('[role="dialog"]').$('button=Fechar').click();
      await $('[role="dialog"]').waitForExist({ reverse: true });
    }
    const median = report('Exportar: .zip do pack grande', times, 5000);
    // Sem meta própria na SPEC: a referência é a de "Atualizar índice"; a cópia dos arquivos
    // para o staging pesa muito no Windows da CI, então a folga é o dobro.
    expect(median).toBeLessThan(e2eLimit(5000, 2));
  });
});
