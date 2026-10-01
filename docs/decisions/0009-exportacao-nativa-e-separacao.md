# ADR-0009 — Exportação nativa do packwiz e separação entre projeto e instância

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

O packwiz indexa tudo o que está na pasta do pack, inclusive lixo (R3 §1.8). Packs populares saem com caches e configs pessoais (R1 §1.B). Os apps anteriores deixavam `.bak`, jars auxiliares e `.packwiz.toml` dentro do pack, que acabavam indexados (R4 §2.3).

## Decisão

- A exportação produz o **formato nativo do packwiz** com **só o necessário**: `pack.toml`, `index.toml` e os arquivos do índice (metafiles, configs, resource packs/shaders locais e o que o usuário adicionou). Saída em pasta ou `.zip`.
- O usuário vê uma **pré-visualização** exata antes de exportar (SPEC T19).
- `.mrpack` e zip da CurseForge ficam para depois (P2; decisão pendente D1).
- **Projeto do pack e instância de teste são pastas separadas.** A instância é derivada do projeto; o caminho inverso passa sempre por revisão (SPEC T15).
- O Warden cria `.packwizignore`, `.gitignore` e `.gitattributes` padrão e faz verificação de higiene ao abrir e ao exportar.

## Alternativas consideradas

- Usar a pasta do pack como instância (modelo Enigmatica/Craftoria com InstanceSync): mistura dados de execução com o pack e exige ignores perfeitos.
- Exportar a pasta inteira: leva o que estiver lá.

## Consequências

- O Warden nunca escreve na pasta do pack nada que não seja conteúdo (ARCHITECTURE §6.4–§6.5).
- Metadados do Warden ficam em `.warden/`, ignorado pelo packwiz (ADR-0021).
