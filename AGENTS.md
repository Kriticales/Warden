# Warden: contexto comum para os agentes

Você trabalha para o **orquestrador** do projeto Warden. O orquestrador não escreve código: ele instrui, revisa e integra. O dono do projeto é leigo em programação e exige qualidade máxima, funcionamento real e código muito bem feito.

Se você é o orquestrador, leia também `docs/ORQUESTRADOR.md`.

## O que é o Warden

App desktop pessoal e privado (Tauri 2: backend Rust + interface React/TypeScript; Windows primeiro) para criar, editar, versionar, testar e diagnosticar modpacks de Minecraft no formato **packwiz**, com launcher offline embutido e diagnóstico com IA (Gemini). Repositório GitHub privado: `Kriticales/Warden`. Sucessor dos protótipos `Kriticales/packwiz-gui` e `packwiz-gui-manager`, que servem só de referência.

## Onde está cada coisa

- `docs/SPEC.md`: o que o app faz, tela por tela, com critérios de aceite (CA-Txx-yy). §9 lista o que fica fora; §10 tem as decisões do dono (D1–D33).
- `docs/ARCHITECTURE.md`: crates, módulos e contratos.
- `docs/ROADMAP.md`: as tarefas (F0-xx, P1-xx, L-xx, D-xx, A-xx, V-xx, E-xx, W-xx...), com dependências, posse de arquivos, critérios e verificação, em ondas. Marcos M1–M5 (v1) e M6 (Warden 1.1 "Profissional").
- `docs/QUALITY.md`: padrões de código, testes, cobertura, glossário (termos da interface) e privacidade.
- `docs/decisions/`: ADRs 0001–0047. Não contrarie uma ADR sem registrar uma nova.
- `docs/research/01–06`: pesquisas (boas práticas, launcher, packwiz/APIs, projetos anteriores, diagnóstico avançado, criação/edição/descoberta). Leia as seções "Implicações para o Warden" do que for relevante à sua tarefa.
- `docs/spikes/S1-motor-do-launcher.md`: escolha do motor do launcher. O código do spike está na branch `spike/launcher-engine`.
- `docs/design/`: `ESTRUTURA.md` (navegação aprovada), `DESIGN-SYSTEM.md`, `HANDOFF.md`.
- `design/system/` (tokens, componentes, fontes) e `design/prototipo-final/` (protótipo navegável aprovado).

## Decisões que não se reabrem

- Tauri 2 + React/TypeScript. Windows é a plataforma principal; o app também compila em Linux (CI).
- Uso pessoal e privado. Licenças de terceiros registradas em `THIRD_PARTY.md`.
- Minecraft 1.7.10 até a mais nova. Forge (todas), NeoForge (1.20.1+), Fabric (1.14+). Quilt e Legacy Fabric depois.
- packwiz **híbrido**: o Warden lê e escreve `pack.toml`/`index.toml`/`.pw.toml` em Rust e usa o binário do packwiz (sidecar, commit fixado) para refresh, validação e exportação.
- Launcher próprio, **só perfil offline**, motor **portablemc 5.0.5** atrás da trait `LauncherEngine`; o Warden controla processo, logs e diagnóstico.
- Fontes: Modrinth + CurseForge (busca combinada) + arquivo local/URL.
- Exportação packwiz, `.mrpack` e zip da CurseForge. Publicação no GitHub com changelog automático; jogadores usam o packwiz-installer-bootstrap pelo link do `pack.toml`.
- IA Gemini com a chave do usuário: consentimento uma vez por conversa, envios visíveis, nada muda sem **Aplicar**.
- Interface toda em português do Brasil. Navegação: "Meus packs" → pack com 6 seções (Mods, Configs, Problemas, ✦ Diagnóstico com IA, Histórico, Exportar), **sem abas**.
- Visual "Deep Dark" (cores do mob Warden, fontes pixel, menus estilo Minecraft). Nada com "cara de IA" (gradiente roxo, glassmorphism, emojis, texto de marketing); o ícone ✦ é o da IA.

## Segredos

- A chave da CurseForge fica no `.env` da raiz do repositório principal (`CURSEFORGE_API_KEY`, entre aspas simples porque começa com `$2a$`). **Nunca** imprima, registre em log, copie para arquivo versionado ou faça commit dela. Use um parser de `.env` que respeite aspas simples.
- Se o seu worktree não tiver o `.env`, leia o do repositório principal: `C:\Users\solel\orca\projects\Warden\.env`.
- No app final as chaves (CurseForge, Gemini, GitHub) são digitadas pelo usuário e guardadas no cofre do Windows (opção `.env` na pasta de dados do app, ADR-0025).

## Regras de trabalho

- Trabalhe só no seu worktree e na sua branch. Não toque no `main`, não faça merge nem push, salvo se a tarefa mandar.
- Commits pequenos e bem descritos, em português. Termine cada mensagem com a linha `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- O git deste repositório usa `core.autocrlf=false`. Não mude.
- Documentação em português do Brasil com acentuação correta; identificadores e termos técnicos no original.
- Não amplie o escopo. Achou algo importante fora dele? Registre no relatório final.
- Seja honesto: diga o que verificou executando e o que é inferência.
- Subagentes: só se o orquestrador liberar na tarefa, com o limite que ele der. Cada subagente edita arquivos distintos, na sua branch; você revisa antes de cada commit.
- Design: publique protótipos e páginas para o dono como **Artifact do claude.ai** (privado), não no Superset. O orquestrador confere antes de mostrar ao dono.

## Envelope de conclusão (obrigatório)

Sua resposta final termina com exatamente um destes blocos:

```
SUPERSET_WORKER_DONE
task: <id da tarefa>
summary: <resultado em uma linha>
files: <caminhos>
checks: <comandos e resultados>
handoff: <contexto para o próximo passo ou none>
```

ou

```
SUPERSET_WORKER_BLOCKED
task: <id da tarefa>
reason: <bloqueio específico>
needs: <decisão, acesso ou dependência necessária>
```

O nome `SUPERSET_` ficou da ferramenta anterior e continua valendo como marcador. Se a tarefa vier pelo `orca orchestration` (com Task e Dispatch no preâmbulo), siga também o preâmbulo e mande o `worker_done` como ele pede.
