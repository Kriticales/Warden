/**
 * E2E com WebdriverIO + `tauri-driver` contra o app real, build de debug (QUALITY §4.1 e
 * §13.5).
 *
 * Antes: `cargo xtask e2e-driver` (driver nativo: `msedgedriver` da versão do WebView2 no
 * Windows, `WebKitWebDriver` no Linux) e `cargo tauri build --debug --no-bundle`. O script
 * `pnpm e2e` faz os dois.
 *
 * Isolamento (QUALITY §13.6): o app roda com `WARDEN_DATA_ROOT` e o cofre de teste em arquivo
 * (`WARDEN_SECRET_BACKEND=file:`) numa pasta temporária, apagada no fim, e com as APIs
 * externas apontadas para o servidor local de fixtures (`e2e/mock-server/`). Nada toca nas
 * pastas reais do Warden nem no cofre do Windows.
 */
import { spawn, type ChildProcess } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createConnection } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { startMockServer, type MockServer } from './e2e/mock-server/server';

const ROOT = resolve(import.meta.dirname, '..', '..');
const TARGET = process.env.CARGO_TARGET_DIR ?? join(ROOT, 'target');
const EXE = process.platform === 'win32' ? '.exe' : '';
const APPLICATION = join(TARGET, 'debug', `warden-app${EXE}`);
const NATIVE_DRIVER_FILE = join(TARGET, 'e2e', 'native-driver.txt');
const DRIVER_PORT = 4444;
/** O jogo simulado dos testes da `warden-launcher` (Testar, L-04). */
const FAKE_GAME_DIR = join(ROOT, 'crates', 'warden-launcher', 'tests', 'fixtures', 'java');

let tauriDriver: ChildProcess | null = null;
let mockServer: MockServer | null = null;
let dataRoot: string | null = null;

function nativeDriver(): string {
  if (!existsSync(NATIVE_DRIVER_FILE)) {
    throw new Error('Driver nativo ausente: rode `cargo xtask e2e-driver` antes.');
  }
  return readFileSync(NATIVE_DRIVER_FILE, 'utf8').trim();
}

/**
 * O Java do jogo simulado (`WARDEN_TEST_JAVA` ou o de `JAVA_HOME`) e o major dele, lido do
 * arquivo `release`. Sem Java, o E2E do Testar avisa e pula.
 */
function testJava(): { java: string; major: string } | null {
  const explicit = process.env.WARDEN_TEST_JAVA;
  const home = process.env.JAVA_HOME;
  const java = explicit ?? (home ? join(home, 'bin', `java${EXE}`) : null);
  if (!java || !existsSync(java)) return null;
  let major = '17';
  try {
    const release = readFileSync(join(java, '..', '..', 'release'), 'utf8');
    const version = /^JAVA_VERSION="([^"]+)"/m.exec(release)?.[1] ?? '';
    const [first, second] = version.split('.');
    major = first === '1' ? (second ?? '8') : (first ?? '17');
  } catch {
    // Sem `release`: fica 17, o Java dos testes da CI.
  }
  return { java, major };
}

/** Espera o `tauri-driver` aceitar conexões. */
async function waitForPort(port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const open = await new Promise<boolean>((done) => {
      const socket = createConnection({ host: '127.0.0.1', port });
      socket.once('connect', () => {
        socket.end();
        done(true);
      });
      socket.once('error', () => {
        done(false);
      });
    });
    if (open) return;
    await new Promise((done) => setTimeout(done, 200));
  }
  throw new Error(`tauri-driver não abriu a porta ${String(port)}`);
}

export const config: WebdriverIO.Config = {
  runner: 'local',
  specs: ['./e2e/**/*.e2e.ts'],
  maxInstances: 1,
  hostname: '127.0.0.1',
  port: DRIVER_PORT,
  capabilities: [
    {
      // O tauri-driver repassa ao driver nativo; o app é o "navegador".
      browserName: 'wry',
      'tauri:options': { application: APPLICATION },
      // O tauri-driver só fala o WebDriver clássico; com o BiDi o WebdriverIO procuraria os
      // elementos num contexto que não é o da janela do app.
      'wdio:enforceWebDriverClassic': true,
    } as WebdriverIO.Capabilities,
  ],
  logLevel: 'warn',
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 60_000 },
  waitforTimeout: 15_000,

  onPrepare: async () => {
    if (!existsSync(APPLICATION)) {
      throw new Error(
        `App ausente (${APPLICATION}): rode \`cargo tauri build --debug --no-bundle\`.`,
      );
    }
    dataRoot = mkdtempSync(join(tmpdir(), 'warden-e2e-'));
    mockServer = await startMockServer();
    // Diálogo nativo de pasta (P1-07): o WebDriver não o alcança; no lugar dele, o app lê a
    // pasta escolhida deste arquivo, que os testes escrevem antes de cada escolha.
    const pickFolder = join(dataRoot, 'pasta-escolhida.txt');
    process.env.WARDEN_E2E_PICK_FOLDER = pickFolder;
    process.env.WARDEN_E2E_DATA_ROOT = dataRoot;
    // Testar (L-04): o jogo simulado no lugar do Minecraft, e o "Salvar em arquivo…" do
    // console pelo caminho escrito neste arquivo.
    const saveFile = join(dataRoot, 'arquivo-escolhido.txt');
    process.env.WARDEN_E2E_SAVE_FILE = saveFile;
    const java = testJava();
    process.env.WARDEN_E2E_HAS_JAVA = java ? '1' : '';
    const env = {
      ...process.env,
      WARDEN_DATA_ROOT: dataRoot,
      WARDEN_SECRET_BACKEND: `file:${join(dataRoot, 'cofre-de-teste')}`,
      WARDEN_E2E_MOCK_URL: mockServer.url,
      WARDEN_E2E_PICK_FOLDER: pickFolder,
      // Catálogo de versões (P1-05) pelas respostas gravadas no servidor de fixtures.
      WARDEN_API_BASE_MOJANG: mockServer.url,
      WARDEN_API_BASE_FABRIC: mockServer.url,
      WARDEN_API_BASE_FORGE: mockServer.url,
      WARDEN_API_BASE_NEOFORGE: mockServer.url,
      // Modrinth e CurseForge (P1-08): o inventário e os detalhes nunca vão à internet.
      WARDEN_API_BASE_MODRINTH: `${mockServer.url}/modrinth/v2/`,
      WARDEN_API_BASE_CURSEFORGE: `${mockServer.url}/curseforge/`,
      WARDEN_E2E_SAVE_FILE: saveFile,
      ...(java
        ? {
            WARDEN_E2E_FAKE_GAME: FAKE_GAME_DIR,
            WARDEN_E2E_JAVA: java.java,
            WARDEN_E2E_JAVA_MAJOR: java.major,
          }
        : {}),
    };
    tauriDriver = spawn(
      'tauri-driver',
      ['--port', String(DRIVER_PORT), '--native-driver', nativeDriver()],
      {
        env,
        stdio: ['ignore', 'inherit', 'inherit'],
      },
    );
    await waitForPort(DRIVER_PORT, 15_000);
  },

  onComplete: async () => {
    tauriDriver?.kill();
    await mockServer?.close();
    if (dataRoot) {
      rmSync(dataRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
    }
  },
};
