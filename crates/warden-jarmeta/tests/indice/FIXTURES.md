# Esqueletos dos jars reais para os índices (D-05)

Fixtures dos critérios da tarefa D-05 (QUALITY §4.1: fixtures reais com origem anotada), usadas por `tests/indice_corpus.rs`.

## O que está aqui

- `extras.json`: os jars que o corpus da P1-06 (`../corpus/corpus.json`) não tinha e a D-05 precisa, no mesmo formato (projeto, versão, URL, **sha1**, tamanho, licença, página):
  - ModernFix 5.25.2 para Fabric 1.20.1, o quinto jar do experimento da R5A §5.3 (Sodium, Lithium, Iris, Fabric API e ModernFix);
  - Epic Fight para NeoForge 1.21.1 (`epicfight.mixins.json` no `[[mixins]]` do `neoforge.mods.toml`) e para Forge 1.20.1 (`mixins.epicfight.json` no `MixinConfigs` do manifesto).
- `esqueletos.json`: de cada jar das duas listas, e de cada jar embutido que o loader carrega, só o que o índice usa:
  - `arquivos`: os descritores **sem alteração** (`fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `mcmod.info`, `META-INF/jarjar/metadata.json`, `META-INF/MANIFEST.MF`); texto quando é UTF-8, lista de bytes quando não é;
  - `pacotes`: os pacotes Java com pelo menos uma classe;
  - `mixins`: as configs de mixin declaradas na árvore do jar que existem neste jar;
  - `aninhados`: os embutidos, pelo caminho dentro do jar.

Nenhuma classe, recurso ou config de terceiros é versionada: o teste remonta um jar "esqueleto" (os descritores, uma classe vazia por pacote, as configs como `{}`) e monta o índice de verdade sobre ele.

## Como atualizar

1. Mude o `corpus.json` da P1-06 ou o `extras.json`.
2. `$env:WARDEN_JARMETA_ATUALIZAR_INDICE = '1'; $env:INSTA_UPDATE = 'always'; cargo nextest run -p warden-jarmeta --run-ignored only -E 'test(rede_indice)'` baixa os jars (cache em `target/tmp/jarmeta-corpus`, compartilhado com o corpus da P1-06, sha1 conferido), refaz `esqueletos.json` e os dourados `../snapshots/indice_corpus__<grupo>.snap`.
3. Revise o diff dos dourados antes de commitar.

Sem as variáveis, `cargo xtask test-network` confere que o índice de cada jar real é igual ao do esqueleto, que o `esqueletos.json` versionado é o que sai dos jars reais e que os dourados batem.

## Licenças

As licenças de cada projeto estão no `corpus.json` e no `extras.json`. Os arquivos copiados são descritores de metadados e nomes de pacotes, guardados só para teste num repositório privado (ADR-0004).
