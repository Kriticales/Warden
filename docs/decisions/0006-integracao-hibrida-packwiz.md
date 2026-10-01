# ADR-0006 — Integração híbrida com o packwiz

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (híbrida) + decisão técnica (A1) (detalhes)

## Contexto

O CLI do packwiz não tem saída estruturada, escreve erros no stdout, às vezes sai com 0 em falhas, escolhe o primeiro resultado de busca com `-y`, sobrescreve metafiles em silêncio, não tem comandos para `side`/`option`/`preserve` e falha no `init` com Forge antigo (R3 §1.4–§1.13). Reimplementar tudo arrisca divergir do formato.

## Decisão

O Warden **lê e escreve** `pack.toml`, `index.toml` e `.pw.toml` diretamente, em Rust, no formato exato do packwiz, e usa o **binário do packwiz** para `refresh`, validação (refresh em cópia + zero diferença) e exportação. O pack final é 100% packwiz. Detalhes (ARCHITECTURE §6.1):

- Adição pela busca (Modrinth e CurseForge): o Warden escreve o `.pw.toml` com dados da API.
- **Link da CurseForge passa pelo packwiz** (`curseforge add <url>`), executado numa **cópia de staging** e importado pelo caminho normal do Warden (evita sobrescrita silenciosa e dependências escolhidas pelo packwiz).
- Remover, lado, opcional, fixar, atualizar e `preserve`: escrita direta + `refresh`.
- Nunca usados: `init`, `add` por busca ou com `-y`, `update`, `migrate`, `serve`, `remove`, `pin`, `detect`, `import`.
- Resultado de cada chamada ao packwiz é confirmado relendo os arquivos.

## Alternativas consideradas

- **Só CLI:** herda todos os defeitos acima e não cobre os requisitos.
- **Só escrita direta, sem o binário:** perde a garantia de "é exatamente o que o packwiz produziria".

## Consequências

- Testes dourados comparam cada tipo de arquivo escrito pelo Warden com o que o packwiz real escreve.
- Testes de integração rodam o binário real em toda mudança (ADR-0022).
- Nada importante pode ficar como comentário em `pack.toml`/`index.toml` (o `refresh` apaga).
