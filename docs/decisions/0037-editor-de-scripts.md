# ADR-0037 — Editor de scripts KubeJS e CraftTweaker

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono + técnica (tarefa D4, pesquisa R5B §3)

## Contexto

Scripts KubeJS (`kubejs/`) e CraftTweaker (`scripts/`) já aparecem na árvore de Configs. O autocompletar de verdade do KubeJS depende das tipagens geradas pelo ProbeJS dentro do jogo e de um servidor de linguagem TypeScript; o `tsc` 7 é um binário de 24 a 28 MB com `--lsp --stdio`. A ADR-0020 escolheu CodeMirror 6 e descartou o Monaco.

## Decisão

- **Escopo da v1 (P1):** os arquivos `.js` e `.zs` abrem no mesmo editor de Configs com realce (JavaScript; ZenScript por gramática simples), **trechos prontos** por versão do KubeJS/CraftTweaker, **autocompletar de IDs** (itens, blocos e tags lidos dos jars do pack), **erros vindos do jogo** (logs `logs/kubejs/*.log` e `crafttweaker.log`, e o servidor web local do KubeJS 7 quando o jogo está aberto), **Recarregar no jogo** (servidor web do KubeJS 7 com o token da instância de teste; stdin do servidor local; senão, o comando para copiar) e **Abrir no VS Code**.
- **Depois da v1 (P2):** autocompletar completo com `tsc` 7 `--lsp --stdio` + tipagens do ProbeJS, e servidor de linguagem de ZenScript para 1.12.2.
- O token do servidor web do KubeJS (`kubejs/config/web_server.json`) é segredo: entra no `.packwizignore` padrão, nos "sempre ignorados" da captura e na varredura antes de publicar. `/.probe/`, `/.vscode/` e `/local/kubejs/` também (decisão D21).
- O editor de scripts grava na instância de teste durante o teste, como as configs; "O que mudou durante o teste" leva os scripts para o pack.

## Alternativas consideradas

- TypeScript dentro do WebView (`@valtown/codemirror-ts`): pesado no WebView2 e preso ao TypeScript 6, enquanto o ProbeJS 8 mira o 7.
- Verificação estrita com `checkJs`: o próprio ProbeJS a desliga porque gera falsos positivos.
- Monaco: descartado pela ADR-0020.

## Consequências

- Nova crate `warden-scripts` (ARCHITECTURE §10.2) e tarefa C-07 no ROADMAP.
