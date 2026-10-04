# Warden

App desktop pessoal e privado para criar, editar, versionar, testar e diagnosticar modpacks de Minecraft no formato [packwiz](https://packwiz.infra.link/), com launcher offline embutido e diagnóstico com IA. Feito com Tauri 2 (Rust) e React/TypeScript; Windows primeiro, compilando também em Linux (CI).

O que o app faz está em [`docs/SPEC.md`](docs/SPEC.md); como é construído, em [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); o padrão de qualidade, em [`docs/QUALITY.md`](docs/QUALITY.md); a ordem das tarefas, em [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Pré-requisitos no Windows

Já instalados nesta máquina ([QUALITY §13.1](docs/QUALITY.md#131-pré-requisitos)). Numa máquina nova, instalar programas é decisão do dono:

- Visual Studio Build Tools 2022 com o componente "Desenvolvimento para desktop com C++" (MSVC);
- WebView2 (já vem no Windows 11);
- Rust pelo `rustup` (a versão, 1.98.1, é fixada pelo [`rust-toolchain.toml`](rust-toolchain.toml) e baixada sozinha);
- Node 24 e pnpm 12;
- git, com `core.autocrlf=false` neste repositório (o fim de linha é garantido pelo [`.gitattributes`](.gitattributes));
- Go (para compilar o sidecar do packwiz, a partir da F0-03) e GitHub CLI (`gh`).

Java não é pré-requisito: o launcher baixa o Java do Minecraft.

## Comandos

Toda a automação passa pelo `xtask`, escrito em Rust: os mesmos comandos funcionam no PowerShell do Windows e no Linux da CI.

```powershell
cargo xtask setup        # ferramentas do cargo (versões fixadas) e dependências do pnpm; só no espaço do usuário
cargo xtask dev          # abre o app no Windows (tauri dev), com pastas e cofre de teste próprios
cargo xtask check        # portão de qualidade completo (QUALITY §12)
cargo xtask check --fast # versão rápida, para cada commit
```

Outros: `cargo xtask check-deps` (nenhuma crate de domínio depende do Tauri), `cargo xtask check-docs` (links internos dos documentos), `cargo xtask bindings` e `cargo xtask bindings --check` (o `bindings.ts` gerado do Rust), `cargo xtask coverage` (cobertura mínima da [QUALITY §4.2](docs/QUALITY.md#42-cobertura-mínima-linhas)) e `cargo xtask test-network` (testes contra as APIs reais, com o `.env`).

### O app de desenvolvimento não toca nos seus dados

`cargo xtask dev` define, só para o app aberto por ele:

- `WARDEN_DATA_ROOT` em `%LOCALAPPDATA%\Warden-dev\<pasta do worktree>\`, no lugar das pastas do Warden instalado;
- `WARDEN_SECRET_BACKEND=file:<WARDEN_DATA_ROOT>\cofre-de-teste`, no lugar do Gerenciador de Credenciais do Windows;
- as chaves do `.env` da raiz do repositório principal, sem imprimir os valores.

Essas variáveis só valem em build de debug ([ADR-0048](docs/decisions/0048-desenvolvimento-no-windows.md)).

### Caminhos curtos

O `link.exe` da Microsoft falha com caminhos acima de 260 caracteres (`LNK1104`). Por isso a pasta `target` de cada worktree fica com até 100 caracteres de caminho; `setup`, `dev` e `check` avisam quando passa. A solução é uma pasta `target` curta só daquele worktree:

```powershell
$env:CARGO_TARGET_DIR = 'C:\wt\<nome-do-worktree>'
```

Detalhes na [QUALITY §13.3](docs/QUALITY.md#133-caminhos-longos-e-a-pasta-target).

## Estrutura

```
apps/desktop/            interface React + TypeScript (Vite, TanStack Router, i18next)
apps/desktop/src-tauri/  crate warden-app: a única que conhece o Tauri
crates/                  crates de domínio (warden-core, warden-packwiz, ...)
xtask/                   automação do desenvolvimento (cargo xtask)
docs/                    especificação, arquitetura, qualidade, plano, decisões e pesquisas
design/                  design system e protótipo aprovado
```

Dependências de terceiros e licenças: [`THIRD_PARTY.md`](THIRD_PARTY.md).
