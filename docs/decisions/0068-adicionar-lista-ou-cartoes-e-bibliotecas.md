# ADR-0068 — Adicionar: Lista ou Cartões, bibliotecas escondidas e o que já está no pack nas listas

- **Status:** aceita · **Data:** 2026-10-25 · **Origem:** decisão do dono (relatório de bugs nº 1, problemas 15 e 1; esboços aprovados `design/bug-01/adicionar.html`, opção 4, e `adicionar-itens.html`, opção 4) · **Muda:** SPEC T08 ("listas, nunca grade de cartões") e T09 · **Complementa:** [ADR-0027](0027-busca-combinada.md)

## Contexto

A página Adicionar mostrava 7 mods por tela em linhas altas, com os filtros à esquerda e a pré-visualização sempre aberta à direita, e a SPEC T08 proibia a grade de cartões no início. O dono pediu mais mods por tela e, ao comparar os esboços, escolheu ter **os dois modos**: a tabela densa para comparar números e a grade para bater o olho. Nas listas do início, as bibliotecas (Cloth Config, Architectury, GeckoLib…) ocupavam metade dos populares sem nunca serem o que alguém procura, e o que já estava no pack sumia das listas, o que confundia ("cadê o JEI?"). O diálogo de dependências listava tudo em grupos (escolhidos, obrigatórias, opcionais) sem dizer de quem cada dependência era, e o título e o botão contavam de jeitos diferentes.

## Decisão

1. **Lista | Cartões.** A página tem os dois modos, com o seletor ao lado da busca. Lista é a tabela densa (linhas de ~40 px; nome e autor, resumo, downloads, atualizado, fonte, situação). Clicar em **Nome** ordena o que já foi carregado; **Downloads** e **Atualizado** pedem a busca ordenada às fontes (a ordem continua a da ADR-0027). Cartões é a grade de cartões baixos com **Marcar** e, como não tem colunas, um **Ordenar**. Os filtros, a seleção, as bibliotecas escondidas e o selo No pack são os mesmos nos dois. O que foi marcado continua marcado ao trocar.
2. **O app lembra o modo** no armazenamento local do WebView (`localStorage`, chave `warden.adicionar.modo`). Não é segredo nem dado de API, só uma preferência de tela; sem armazenamento, a página abre na Lista. Não entra em `settings.json` para não criar uma configuração que ninguém vai procurar em Configurações.
3. **Filtros à direita**, iguais nos dois modos: Categoria em lista curta (8 e "Mais N"), com a marca discreta da loja no lugar de "· Só no Modrinth"; Funciona em; Mostrar também (Bibliotecas, O que já está no pack, Mods sem versão para o pack); Fonte. A trava de versão vira uma linha discreta abaixo da busca. Clicar no nome mostra o detalhe do mod **no lugar dos filtros** (a pré-visualização deixa de ocupar uma terceira coluna fixa).
4. **Bibliotecas** são marcadas pelo backend (`SearchResult.is_library`): Modrinth pela categoria `library` da busca, CurseForge pela categoria 421 ("API and Library"), principal ou não; o item combinado é biblioteca se uma das fontes disser. São as mesmas categorias da categoria curada "Bibliotecas" (um teste amarra as duas). A interface as esconde por padrão (uma linha "N bibliotecas escondidas… Mostrar" na Lista; um grupo fechado nos Cartões) e as mostra quando há texto na busca, quando a categoria é Bibliotecas ou com "Mostrar também: Bibliotecas". Elas continuam entrando sozinhas como dependência.
5. **O que já está no pack aparece nas listas iniciais**, com o selo **No pack** e sem caixa. O início traz a página inteira de cada busca (até 20 por lista, `HOME_ITEMS = PAGE_SIZE`) e o total ("17 de 2.481"); sai a nota "também estão entre os mais baixados, mas já estão no seu pack" (`popular_in_pack`). "Mostrar também: O que já está no pack" desligado os esconde.
6. **Diálogo Adicionar itens em árvore + detalhe:** um galho por mod escolhido com o que só ele pede pendurado, "Em comum" uma vez só, o que pede decisão numa linha no topo, o aviso de lado uma vez e o porquê do item clicado à direita. A mesma conta aparece no título, no botão e no rodapé. A regra do que entra continua a de `features/add/dependencies/model.ts` (B1-B). Para "o que faz" e as versões no mesmo formato, o plano passa a trazer o resumo da fonte (`PlanNode.summary`) e o número da versão do Modrinth passa pela mesma limpeza da CurseForge.

## Alternativas consideradas

- **Só a tabela densa (opção 1) ou só a grade (opção 2):** o dono preferiu escolher na hora.
- **Esconder as bibliotecas pela categoria curada, sem campo novo:** a busca não devolve as categorias para a interface, e esconder pela categoria pediria uma busca a mais por lista (o orçamento do início é de 3 requisições por fonte).
- **Lembrar o modo em `settings.json`:** exigiria mexer nas Configurações (outra tarefa) por uma preferência de tela.
- **Dependências conhecidas na barra dos marcados só pelo que a pré-visualização já leu:** deixaria a barra vazia na maioria das vezes. A barra pede o mesmo plano do diálogo quando a seleção para de mudar (600 ms), guardado por 5 minutos e sem nova tentativa em erro.

## Consequências

- A SPEC T08 deixa de dizer "listas (nunca grade de cartões)"; T08 e T09 descrevem os esboços aprovados.
- A pré-visualização completa continua existindo, agora na coluna da direita, no lugar dos filtros.
- O diálogo usa a largura nova `--size-dialog-xl` (1200 px) do design system.
- Marcar itens dispara um plano (`add_plan`) depois de 600 ms sem mudanças, para a barra mostrar as obrigatórias já conhecidas; o diálogo continua pedindo o plano de novo ao abrir.
