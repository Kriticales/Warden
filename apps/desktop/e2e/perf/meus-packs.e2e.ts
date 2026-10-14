/**
 * Desempenho de Meus packs no app real (A-04; CA-T02-01: com 50 packs registrados, a lista
 * aparece em menos de 1 s).
 *
 * Os 50 packs são como os de quem usa o Warden: todos versionados no git, 5 grandes (300 mods e
 * 500 configs) e 45 pequenos, metade com uma versão salva e alterações por salvar. O
 * `packs.e2e.ts` mede a mesma lista com packs vazios e sem git; aqui cada linha lê o
 * `pack.toml` e conta as alterações no git, como no uso de verdade.
 *
 * O app abre (processo novo) e a página recarrega mais 4 vezes; em cada uma, o tempo vai do
 * início da página até as 50 linhas na tela, medido dentro do WebView. A meta vale para a
 * mediana, com a folga do build de debug na CI (`DEBUG_MARGIN`, em `comum.ts`; a CA exata com
 * packs simples continua no `packs.e2e.ts`); a mediana e o pior caso saem no log
 * (`A-04 E2E | …`), contra a meta exata. A primeira abertura também
 * mede, de fora, o tempo de abrir o app até a lista (SPEC §8: < 3 s), só para o log.
 */
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { $, browser, expect } from '@wdio/globals';

import { BIG_PACK, e2eLimit, report, ulid, versionPack, writePack } from './comum';

function env(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} ausente: rode pelo wdio.conf.ts`);
  return value;
}

const DATA_ROOT = env('WARDEN_E2E_DATA_ROOT');
const PACKS_DIR = join(DATA_ROOT, 'cinquenta-versionados');
const SETTINGS_FILE = join(DATA_ROOT, 'config', 'settings.json');
const REGISTRY_FILE = join(DATA_ROOT, 'config', 'packs.json');

/** Quantos packs e quantos deles são grandes. */
const PACKS = 50;
const BIG = 5;
/** Quantas vezes o app reabre para medir. */
const RUNS = 5;

/** Do início da página até `count` linhas de Meus packs na tela, medido dentro do WebView. */
async function listShownAt(count: number): Promise<number> {
  return browser.execute(
    (wanted: number) =>
      new Promise<number>((resolve) => {
        const check = () => {
          if (document.querySelectorAll('tr.packrow').length >= wanted) resolve(performance.now());
          else requestAnimationFrame(check);
        };
        check();
      }),
    count,
  );
}

describe('Desempenho: Meus packs com 50 packs versionados (A-04; CA-T02-01)', () => {
  before(function () {
    // Gerar e versionar 50 packs (5 com 800 arquivos) leva mais de um minuto no Windows da CI.
    this.timeout(300_000);
    rmSync(PACKS_DIR, { recursive: true, force: true });
    const packs = [];
    for (let n = 0; n < PACKS; n += 1) {
      const dir = join(PACKS_DIR, `pack-${String(n).padStart(2, '0')}`);
      const name = `Pack ${String(n).padStart(2, '0')}`;
      writePack(
        dir,
        n < BIG
          ? { ...BIG_PACK, name, resourcePacks: 0, shaders: 0 }
          : { name, mods: 10, resourcePacks: 0, shaders: 0, configs: 20 },
      );
      versionPack(dir, n % 2 === 0);
      packs.push({ id: ulid(n + 1), name, path: dir });
    }
    mkdirSync(join(DATA_ROOT, 'config'), { recursive: true });
    writeFileSync(SETTINGS_FILE, '{ "schemaVersion": 1 }\n');
    writeFileSync(REGISTRY_FILE, JSON.stringify({ schemaVersion: 1, packs }, null, 2));
  });

  after(() => {
    rmSync(SETTINGS_FILE, { force: true });
    rmSync(REGISTRY_FILE, { force: true });
    rmSync(PACKS_DIR, { recursive: true, force: true });
  });

  it('CA-T02-01: lista com 50 packs versionados (mediana de 5; meta de 1 s, com a folga do debug)', async () => {
    const times: number[] = [];
    // A primeira abertura é a do app (processo novo); as seguintes recarregam a página, sem
    // reabrir o processo (reabrir muitas vezes seguidas às vezes derruba o GTK no Xvfb da CI).
    // SPEC §8, abrir o app até a Lista de packs (< 3 s): do pedido de sessão nova ao WebDriver
    // (o tauri-driver abre o processo) até as linhas na tela, visto de fora do app. Inclui a
    // criação da sessão do WebDriver, então é um teto do que a pessoa espera; medida uma vez só
    // (reabrir o processo muitas vezes às vezes derruba o GTK no Xvfb), vai só para o log.
    const launched = Date.now();
    await browser.reloadSession();
    times.push(await listShownAt(PACKS));
    const startup = Date.now() - launched;
    process.stdout.write(
      `A-04 E2E | abrir o app até Meus packs com 50 packs | ${String(startup)} ms | meta < 3000 ms\n`,
    );
    await browser.setWindowSize(1366, 860);
    for (let run = 1; run < RUNS; run += 1) {
      await browser.refresh();
      times.push(await listShownAt(PACKS));
    }
    const median = report('Meus packs: 50 packs versionados (5 grandes)', times, 1000);
    expect(median).toBeLessThan(e2eLimit(1000));

    // As linhas trazem o que vem do git: o pack grande salvo tem 2 alterações por salvar.
    const row = $('tr.packrow*=Pack 00');
    await expect(row).toHaveText(expect.stringContaining('Minecraft 1.20.1 · Fabric 0.16.5'));
    await expect(row.$('[aria-label="2 alterações não salvas"]')).toExist();
  });
});
