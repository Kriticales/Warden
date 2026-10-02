# ADR-0035 — Importar modpacks de outros apps e gerar o pacote para servidor

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (tarefa D4, pesquisa R5B §5.1 e §5.2)

## Contexto

O dono quer trazer packs feitos em outros apps e entregar o pack também para quem roda servidor. O packwiz só importa da CurseForge (e marca tudo como "cliente e servidor"); `.mrpack` e instâncias do Prism ficam por conta do Warden. `.mrpack` reais trazem lixo (`.mixin.out/`, `xmcl.json`, `mods/.connector/temp`) e `env: "unknown"`, que não existe na especificação.

## Decisão

- **Importar (P1, antes P2):** "Abrir ou importar…" aceita pasta packwiz, `.mrpack`, zip da CurseForge e instância do Prism/MultiMC ou do app da CurseForge. `.mrpack` e Prism são convertidos pelo Warden; o zip da CurseForge passa pelo `packwiz curseforge import` em staging (exige a chave da CurseForge), com o lado corrigido pelo Modrinth quando o mesmo arquivo existir lá. Tudo passa pela verificação de higiene antes de gravar; jars soltos são identificados por hash e viram referência quando possível; `env` desconhecido vira "Cliente e servidor" com o aviso de lado desconhecido; a licença do projeto aparece quando não for livre. O resultado é um pack novo, nunca altera o arquivo de origem.
- **Pacote para servidor (P1):** novo formato em Exportar, com **duas variantes**. **Padrão: baixar os mods pelo link do pack** (scripts que rodam o packwiz-installer-bootstrap com `-s server` antes de iniciar; exige o pack publicado; o servidor se atualiza a cada versão publicada). **Alternativa: mods dentro do zip** (funciona sem internet; o Warden avisa sobre licenças e sobre mods da CurseForge que exigem download manual). As duas levam `user_jvm_args.txt`, `start.bat`, `start.sh`, configs do servidor e nada de cliente; a EULA é perguntada pelo script ao dono do servidor, nunca aceita pelo Warden. Depois de gerar, o Warden oferece validar no servidor local (ADR-0032).

## Alternativas consideradas

- Deixar importação para depois da v1: inviabiliza "trazer mods de um modpack" e "criar a partir de um modpack" de forma coerente.
- Só a variante com mods dentro: redistribui jars de terceiros e não se atualiza.
- Só a variante pelo link: não funciona para quem não publica.

## Consequências

- Nova crate `warden-import` (ARCHITECTURE §12.2); `warden-export` ganha o módulo `server_pack` (ARCHITECTURE §12.1).
- O validador de `.mrpack` da exportação aceita `env: "unknown"` na leitura e nunca o gera.
