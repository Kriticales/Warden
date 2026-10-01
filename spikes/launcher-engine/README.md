# Spike S1 — motor do launcher (código descartável)

Protótipos usados para escolher o motor que o Warden usa para instalar e abrir o
Minecraft em modo offline. O relatório com resultados e recomendação está em
[`docs/spikes/S1-motor-do-launcher.md`](../../docs/spikes/S1-motor-do-launcher.md).

Nada aqui é código de produção. Nenhum dado do jogo (Java, jars, assets) fica no
repositório: tudo vai para `~/.local/share/warden-spike/` (Linux) ou
`%LOCALAPPDATA%\warden-spike\` (Windows).

## Conteúdo

| Pasta | O que é |
|---|---|
| `pmc-proto/` | Protótipo sobre a crate [`portablemc`](https://crates.io/crates/portablemc) 5.0.5 (Apache-2.0). Contém o esboço da trait `LauncherEngine` (`src/engine.rs`) e o supervisor de processo independente do motor (`src/supervisor.rs`). |
| `theseus-proto/` | Protótipo sobre o app-lib do Modrinth App (`theseus`, GPL-3.0-only), compilado a partir de um checkout local de `modrinth/code`. |
| `scripts/xvfb.sh` | Sobe um Xvfb sem root (baixa os `.deb` com `apt-get download` e extrai com `dpkg -x`). |
| `scripts/screenshot.py` | Captura a tela do Xvfb (xwd) e converte para PNG sem dependências. |

## Como reproduzir (WSL2, sem DISPLAY)

```bash
# 1. Servidor X virtual (deixe rodando em outro terminal)
spikes/launcher-engine/scripts/xvfb.sh 77
export DISPLAY=127.0.0.1:77
export ALSOFT_DRIVERS=null   # evita que o OpenAL use o áudio do WSLg

# 2. portablemc
cd spikes/launcher-engine/pmc-proto
cargo build --release
./target/release/pmc-proto fabric-1.20.1 --screenshot
./target/release/pmc-proto forge-1.12.2 --screenshot
./target/release/pmc-proto neoforge-1.21.1 --screenshot
./target/release/pmc-proto forge-1.7.10 --screenshot
./target/release/pmc-proto forge-1.16.5 --screenshot
./target/release/pmc-proto vanilla-1.20.1 --cancel-before-download --install-only   # cancelamento
./target/release/pmc-proto erro-forge-inexistente --install-only                    # mensagem de erro

# 3. theseus (precisa de um JDK 17+ para o Gradle que o build.rs do app-lib executa)
git clone https://github.com/modrinth/code ~/.local/share/warden-spike/src/modrinth-code
git -C ~/.local/share/warden-spike/src/modrinth-code checkout e0bd3bc
cp ~/.local/share/warden-spike/src/modrinth-code/packages/app-lib/.env.prod \
   ~/.local/share/warden-spike/src/modrinth-code/packages/app-lib/.env
cd spikes/launcher-engine/theseus-proto
JAVA_HOME=<jdk17+> SQLX_OFFLINE=true cargo build
./target/debug/theseus-proto fabric-1.20.1
```

No Windows, o `pmc-proto` compila com a toolchain MSVC (`cargo build --release`) e roda
com `set WARDEN_SPIKE_DATA=%LOCALAPPDATA%\warden-spike\pmc`.

## Origem de terceiros

- `portablemc` — https://github.com/theorzr/portablemc, Apache-2.0, versão 5.0.5 do crates.io.
- `theseus` (app-lib) — https://github.com/modrinth/code, `packages/app-lib`, GPL-3.0-only,
  commit `e0bd3bc` (2026-10-01). Usado só como dependência local neste spike.
- Xvfb e x11-apps — pacotes do Ubuntu 26.04 (`xvfb`, `x11-apps` e dependências), extraídos
  localmente, sem instalação no sistema.
