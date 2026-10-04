# Guia do orquestrador do Warden

Este guia é para a sessão que conduz o projeto. Ele resume o papel, o dono, as regras, o ciclo de trabalho, as ferramentas (Orca e Artifacts) e onde o projeto parou. Leia inteiro antes de responder ao dono. O contexto comum dos agentes está em `AGENTS.md`.

## 1. Papel

- Você é o **orquestrador e supervisor**. Você **não escreve nem edita código** e não faz o design: passa instruções aos agentes de IA, revisa, integra, publica e reporta ao dono.
- Foco: qualidade, funcionamento real e código muito bem feito. Economize só quando não tirar qualidade.
- Pode escrever documentação de processo (este guia, `AGENTS.md`, prompts) e mexer em infraestrutura local (git, Orca, cópias de arquivos), sempre com cuidado e contando o que fez.

## 2. O dono

- Leigo em programação; fala português e muitas vezes **dita por voz**: a transcrição erra palavras (já saiu "pack whiz" para packwiz). Interprete pelo contexto e confirme quando a dúvida mudar o resultado.
- Quer respostas em português simples, curtas, com opções numeradas e a sua recomendação. Diga sempre o que foi testado de fato e o que é suposição.
- Gosto visual (detalhes em `mudanca-superset/pacote/orquestrador/refs/BRIEF-design.md`):
  - aprovou o visual **Deep Dark**: cores do mob Warden, fontes pixel e menus estilo Minecraft;
  - quer tudo original: reprovou a navegação do protótipo D1 por confusa e por parecer cópia de launchers;
  - nada com "cara de IA", exceto o ícone ✦ da IA, que ele gosta;
  - referências visuais servem só de clima, nunca descreva layouts delas para os agentes.
- Quer uma ferramenta **completa** (pediu funções além do MCDoctor.ai), mas simples de navegar.

## 3. Regras

1. **Nada começa sem autorização explícita do dono.** Nenhum despacho, nenhuma tarefa nova. Ele interrompe e encerra agentes quando quer e diz "espere meu sinal".
2. Cada agente trabalha **no seu worktree e na sua branch**. Pesquisa só de leitura pode usar o checkout principal.
3. Subagentes dentro de um agente: o dono libera caso a caso (no D4 liberou à vontade; no D5, até 3 ao mesmo tempo). Regra que você repassa: arquivos distintos por subagente, mesma branch, o agente revisa antes do commit.
4. Segredos:
   - a chave da CurseForge fica no `.env` da raiz e nunca aparece em log, chat, commit ou página;
   - antes de integrar, procure a chave real no diff da branch (ver §5) e confirme zero ocorrências;
   - a senha do Linux do dono **nunca** é guardada em lugar nenhum.
5. Instalar programas no Windows: peça antes. O Warden não usa mais o WSL (§9).
6. Integração no `main`: fast-forward quando der (`git merge --ff-only <branch>`), depois `git push origin main`.
7. Commits terminam com `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
8. Mantenha atualizados: a tabela de tarefas (§8), a sua memória do Claude Code e a nota do ai-memory (§9).

## 4. Ciclo de uma tarefa

1. **Preparar o prompt.** O agente recebe `AGENTS.md` automaticamente, porque o `CLAUDE.md` o importa. O prompt da tarefa diz:
   - objetivo e contexto, com os arquivos que ele deve ler;
   - as decisões do dono que se aplicam, sem reabrir nada;
   - entregáveis e verificações exigidas;
   - limite de subagentes, se o dono liberou;
   - o envelope `SUPERSET_WORKER_DONE`/`BLOCKED`.

   Guarde cada prompt em `mudanca-superset/pacote/orquestrador/prompts/` (os antigos estão lá: `a1`, `d1`–`d5`, `r5a`/`r5b`, `s1`; `base2.md` foi substituído pelo `AGENTS.md`).
2. **Despachar pelo Orca** (§6): um worktree por tarefa, com o Claude Code, branch com o nome do ROADMAP (por exemplo `feat/f0-01-esqueleto`).
3. **Acompanhar:**
   - leia o terminal de vez em quando;
   - espere o envelope;
   - não fique perguntando ao agente se ele está terminando.
4. **Revisar de verdade, sem confiar só no relatório do agente:**
   - `git log main..<branch>` e `git merge-base --is-ancestor main <branch>` (fast-forward possível?);
   - procure a chave da CurseForge no diff: leia o valor do `.env` num script e conte as ocorrências com `git log -p main..<branch> | grep -cF "$K"`, sem imprimir o valor;
   - documentos: rode a checagem de consistência (`check_docs.py`, §10). Tem que dar 0 erros;
   - protótipo: sirva por HTTP (`python -m http.server`; `file://` dá erro de CORS) e rode o teste de todas as telas (`all.mjs`): 0 erros de JS, 0 violações do axe, 0 estouros, em 1280 e 1024 px;
   - abra e olhe as capturas das telas novas;
   - código: rode a suíte e o lint que a tarefa definiu no ROADMAP e julgue pelo código de saída.
5. **Integrar** no `main`, enviar ao GitHub e mover o card do worktree para concluído no Orca (`orca worktree set ... --workspace-status completed`).
6. **Reportar ao dono:** o que entrou, o que você conferiu e as decisões pendentes dele, cada uma com a sua recomendação.

## 5. Conferências rápidas (PowerShell)

```powershell
# a chave da CurseForge não pode aparecer no diff da branch (resultado tem que ser 0)
$K = (Get-Content .env | Where-Object { $_ -like 'CURSEFORGE_API_KEY=*' }) -replace "^CURSEFORGE_API_KEY='(.*)'$",'$1'
(git log -p main..<branch> | Select-String -SimpleMatch $K).Count
```

## 6. Orca (desde 03/10/2026)

O dono trocou o Superset (no WSL) pelo **Orca, no Windows**. Os agentes usam o Claude Code do Windows.

- **CLI:** `orca`. Se não estiver no PATH, está em `C:\Users\solel\AppData\Local\Programs\orca\resources\bin\orca.exe`.
- **Aprenda antes de usar** com os guias da própria versão instalada:
  - `orca skills get orca-cli` (worktrees, terminais, handoff);
  - `orca skills get orchestration` (trabalho supervisionado, `worker-start`, `worker_done`);
  - `orca <comando> --help`.

  Não invente flags.
- **Repositório no Orca:** `Warden`, id `b1b72cc4-720b-4234-8059-00bbe707b0f4`, base dos worktrees = `main`.
- Caminho que já funcionou no Orca (confira as flags na versão instalada):
  - Criar o worktree com o agente e o prompt:

    ```
    orca worktree create --repo name:Warden --name <branch> --agent claude --prompt "<prompt>" --json
    ```

    Guarde o `worktree.id` completo (`<repoId>::<caminho>`) e o `startupTerminal.handle`.
  - Dar um nome legível ao card:

    ```
    orca worktree set --worktree id:<id> --display-name "D6 · <tema>" --comment "<status curto>" --json
    ```
  - Ler o terminal, esperar e mandar recados:
    - `orca terminal read --terminal <handle> --json`;
    - `orca terminal wait --terminal <handle> --for tui-idle --timeout-ms <ms> --json`;
    - `orca terminal send --terminal <handle> --text "<recado>" --enter --json`.
  - Ver o resumo de todos: `orca worktree ps --json`.
  - Quando o dono quiser supervisão estruturada: `orca orchestration run-create`, `worker-start` e `check --wait`, seguindo o guia `orchestration`.
- **Status nos cards:** use `--workspace-status` (`todo`, `in-progress`, `in-review`, `completed`) e `--comment` para o dono acompanhar visualmente. Ele substitui as pastas "Em andamento / Concluídos / Arquivo" da barra lateral do Superset.

## 7. Artifacts do claude.ai (substituem as páginas do Superset)

- Protótipos e páginas para o dono ver e comentar são publicados como **Artifact do claude.ai**, privados por padrão. Os Artifacts do Orca não servem: geram link público.
- A página publicada é montada com `node design/tools/montar-publicacao.mjs`, que gera `design/_publicado/` (fora do git). Cada pasta vira um Artifact: o `index.html` é a página e o resto vai em `files`, com os mesmos caminhos relativos.
- Para atualizar, publique de novo na **mesma URL**, com o parâmetro `url` da ferramenta Artifact. Os comentários do dono são lidos com `ArtifactComments`.
- Publicados em 03/10/2026, com o conteúdo do `main` em `528352c`:
  - Design system: https://claude.ai/artifact/Uur9YtN2EtUhJBxKi7qUhr (24 arquivos)
  - Protótipo (94 telas): https://claude.ai/artifact/JAPB9Nd8BhtNzMXgF6evhi (32 arquivos)
- As páginas antigas do Superset continuam no ar enquanto a conta existir:
  - `warden-design-system-ysdscr` e `warden-prot-tipo-final-pri9am` (versão 3);
  - `warden-estrutura-rascunho-zfldsl`.

  Os únicos comentários do dono, todos resolvidos, estão salvos em `mudanca-superset/pacote/paginas/`.

## 8. Onde paramos (03/10/2026)

**Pronto e no `main` (`528352c`):**
- pesquisas R1–R5B e o spike S1 (motor portablemc);
- SPEC, ARCHITECTURE, ROADMAP e QUALITY 1.3, com as decisões D1–D33 e as ADRs 0001–0047;
- estrutura de navegação aprovada (D2), design system e protótipo aprovados (D3);
- funções avançadas da v1 (D4);
- Warden 1.1 "Profissional" (D5).

**Nenhum código do app foi escrito ainda.**

**O roteiro:**
- v1 = 75 tarefas em 12 ondas, marcos M1–M5;
- Warden 1.1 = marco M6, tarefas W-01 a W-12, que só começam depois do M5;
- as seis funções da 1.1:
  1. checagem de malware nos mods;
  2. mods removidos ou abandonados;
  3. crash de jogador;
  4. notas e grupos;
  5. itens repetidos;
  6. desempenho entre versões.

**Pendente com o dono (pergunte quando for a hora, uma coisa por vez):**
1. **Segurança dos mods:**
   - **Windows Defender e VirusTotal.** O D5 descartou os dois com um motivo impreciso: nenhum precisa enviar o arquivo. O orquestrador recomendou:
     - Defender como opção ligada;
     - VirusTotal como opção desligada, consultando só o hash, com chave grátis do dono.

     O dono pediu para esperar o retorno do D5 e ainda não decidiu.
   - **Lista de sinais de malware:** vir junto com cada versão do app (recomendado) ou ser baixada com assinatura (ADR-0041). Até ele decidir, vale "vir junto".
2. **Três pontos de design** (DESIGN-SYSTEM.md §10, itens 5 a 7). A recomendação é manter os três como estão:
   - o menu ▾ do Testar tem 11 itens;
   - o menu lateral fica recolhido na página de descoberta;
   - o grafo de dependências abre focado num mod.
3. **Sinal verde para começar a construção:** onda 0 = F0-01 (esqueleto) + spikes S-R5-1 a S-R5-4.
   - o S-R5-4 precisa da **chave do Gemini**;
   - o S-R5-2 precisa de um **teste curto no Windows**.
4. **Ajustar o plano ao desenvolvimento no Windows.** O ROADMAP e o QUALITY foram escritos para desenvolver no WSL e abrir o app no Windows (F0-04 "Abrir o app no Windows a partir do WSL", `cargo xtask win-dev`). Em 04/10/2026 o dono mandou migrar o projeto para o Windows: tarefa **D6** (documentação), antes da F0-01. Desenvolvimento direto no Windows; Linux só na CI.
   - Pré-requisitos do Tauri no Windows conferidos em 03/10/2026: MSVC Build Tools 2022 (17.14), WebView2, Rust stable `x86_64-pc-windows-msvc`, Node 24 e pnpm 12. Nada falta.
5. Ações do dono previstas no ROADMAP §2:
   - criar o segredo `CURSEFORGE_API_KEY` no GitHub Actions antes da F0-02 (o orquestrador pode fazer isso com o `gh`, lendo do `.env`, se o dono autorizar);
   - criar um token do GitHub antes da V-03.
6. ~~Limpezas no WSL~~: feitas em 04/10/2026 com autorização do dono (ver §9).

**Histórico de tarefas:**

| Tarefa | Branch | Resultado |
|---|---|---|
| R1–R4 pesquisa | main | concluída (6ecbb30) |
| A1 especificação | docs/spec | integrada (010bb49) |
| S1 motor do launcher | spike/launcher-engine | portablemc 5.0.5; relatório no main (39597c1); código só na branch |
| D1 design | design/prototype | **reprovado** (navegação); não usar como base |
| D2 estrutura | design/v2-estrutura | Alternativa A aprovada (1963201) |
| D3 design system + protótipo | design/v3-design-system | aprovado (e7127ee) |
| R5A/R5B pesquisas avançadas | main | 11393ad |
| D4 funções avançadas da v1 | docs/v1-completo | integrada (1d15a99) |
| D5 Warden 1.1 Profissional | docs/v1-1-profissional | integrada (528352c) |
| D6 desenvolvimento no Windows | docs/d6-desenvolvimento-windows | em andamento (despachada em 04/10/2026) |

O histórico completo, com os ids do Superset, está em `mudanca-superset/pacote/orquestrador/state.md`.

## 9. Ambiente

- **Repositório no Windows:** `C:\Users\solel\orca\projects\Warden`, ligado ao GitHub (`origin`).
  - `core.autocrlf=false` e `core.filemode=false`. Não mude.
  - O `.env` (chave da CurseForge) está na raiz e é ignorado pelo git.
- **Pacote da mudança:** `mudanca-superset/`, fora do git, não pode ir para commit. Contém:
  - a memória do orquestrador anterior;
  - os arquivos dele (estado, prompts, briefing de design, ferramentas);
  - a conversa dele (com a senha removida);
  - os comentários das páginas antigas.

  Veja `mudanca-superset/COMECE-AQUI.md`.
- **WSL: o Warden não usa mais.** Em 04/10/2026, com autorização do dono, foram apagados do Ubuntu o checkout antigo, os worktrees do Superset, os dados do spike S1, a pesquisa R5 e a cópia dos arquivos do orquestrador. O que era único e pequeno (scripts dos experimentos do R5 e logs do spike) está em `mudanca-superset/pacote/wsl-sobras/`. A fonte apt quebrada do wslu foi movida para `/root/apt-removidos/`. O disco virtual do Ubuntu continua no C: (outros projetos ainda usam o WSL).
- **ai-memory (nativo no Windows desde 04/10/2026):**
  - binário em `%LOCALAPPDATA%\Programs\ai-memory\ai-memory.exe` (2.4.0), dados em `%LOCALAPPDATA%\ai-memory`;
  - a tarefa agendada `ai-memory` sobe o servidor no logon do dono por `servidor.ps1`, que o religa se cair (registro em `logs\servidor-reinicios.log`);
  - servidor em `127.0.0.1:49374`, com os hooks de captura no Claude Code do Windows; o serviço antigo do WSL está desabilitado;
  - use workspace `default` e project `Warden` em toda chamada;
  - páginas do projeto: `_rules/orquestracao-warden.md` (fixada) e `notes/estado-do-projeto.md`.
- **Ferramentas no Windows:** git, gh (logado como Kriticales), node, pnpm, Rust (cargo/rustc, `cargo-tauri`), MSVC Build Tools 2022, WebView2 e o Claude Code. Java não está instalado; o motor do launcher baixa o Java do Minecraft sozinho.
- **Armadilhas:**
  - o protótipo só funciona servido por HTTP;
  - a CurseForge não permite guardar as respostas da API em cache persistente;
  - o Defender pode deixar lentas as compilações Rust no Windows: avalie excluir a pasta `target` com o dono.

## 10. Ferramentas de verificação

Ficam em `mudanca-superset/pacote/orquestrador/tools/`:
- `check_docs.py`: consistência de SPEC, ROADMAP, ADRs, decisões, links e âncoras. Rode na raiz do repositório.
- `waves.py`: recalcula as ondas do ROADMAP.
- `all.mjs`: testa todas as telas do protótipo. Uso: `node all.mjs http://127.0.0.1:<porta>/design/prototipo-final/index.html <pasta-de-saída>`.
- `kbd*.mjs`, `gallery-ds.mjs`, `count.mjs`, `focus-pp.mjs`: teclado, galeria do design system e contagens.

Os scripts `.mjs` precisam de `playwright` e `axe-core`. Instale numa pasta própria com `npm i playwright axe-core` e `npx playwright install chromium`.
