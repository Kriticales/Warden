# Corpus de jars reais da `warden-jarmeta`

Fixtures do critério 1 da tarefa P1-06 (QUALITY §4.1: fixtures reais com origem anotada).

## O que está aqui

- `corpus.json`: um item por jar, com o nome do caso, o loader e a versão do Minecraft a que se destina, a fonte (Modrinth ou CurseForge), o projeto, a versão, o arquivo, a URL de download, o **sha1**, o tamanho, a licença declarada na plataforma e a página da versão. `semDescritor` explica os jars reais que não trazem descritor que o loader leia.
- `jars/<nome>/`: os arquivos de metadados **copiados sem alteração** do jar real (`fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `mcmod.info`, `META-INF/jarjar/metadata.json`, `META-INF/MANIFEST.MF` e os `META-INF/services/` dos loaders) e `_estrutura.json`, gerado pelo teste, com a maior versão de classe e a lista de jars embutidos. Cada jar embutido lido vira uma subpasta curta (`e0`, `e1`...), com o mesmo formato; o caminho real dentro do jar fica no `_estrutura.json` (pastas com o caminho real passariam do limite de 260 caracteres do Windows nos embutidos de embutidos).
- `.gitattributes`: os arquivos extraídos ficam com os bytes originais (sem conversão de fim de linha).
- Os modelos esperados (dourados) ficam em `../snapshots/corpus__<nome>.snap`.

Os jars em si **não** são versionados: são de terceiros. Só os metadados, que são pequenos e necessários para o teste.

## Cobertura

59 jars, escolhidos em 2026-10-04 (versão mais recente de cada projeto para aquele Minecraft e loader):

| Grupo | Jars |
|---|---|
| Fabric 1.20.1 e 1.21.1 | Fabric API (53 módulos embutidos), Sodium, Lithium, Iris (bibliotecas embutidas), Mod Menu, Cloth Config, Architectury, Krypton, FerriteCore, Fabric Language Kotlin, AppleSkin |
| Quilt 1.20.1 | QFAPI/QSL, Quilt Kotlin Libraries |
| Forge 1.7.10 | UniMixins, Angelica, Hodgepodge, GTNHLib, ArchaicFix (Modrinth); NotEnoughItems, CodeChickenCore, Baubles, Applied Energistics 2 rv3, Thaumcraft 4, BuildCraft 7 (CurseForge) |
| Forge 1.12.2 | AppleSkin, JEI, MixinBooter, VintageFix (Modrinth); Baubles, Quark, FoamFix (CurseForge) |
| Forge 1.16.5 e 1.18.2 | JEI, Botania, Curios, AppleSkin; Create 1.18.2 (primeiro jarjar) |
| Forge 1.20.1 | Create (jarjar), JEI, Curios, Supplementaries, BadOptimizations (multi-loader), Packet Fixer *merged* (multi-loader), Sinytra Connector (mod em `Embedded-Dependencies-Mod`), Forgified Fabric API, Radium, Kotlin for Forge (biblioteca com jarjar), GeckoLib, Embeddium |
| NeoForge 1.21.1 | Sodium (mod no jar embutido), Iris, Sinytra Connector, Forgified Fabric API, JEI, Curios, Mekanism, Create, XaeroPlus, Kotlin for Forge |

## Como atualizar

1. Mude o `corpus.json` (os scripts usados para escolher as versões ficaram fora do repositório; basta preencher URL e sha1 a partir da API do Modrinth ou da CurseForge).
2. `$env:WARDEN_JARMETA_ATUALIZAR_CORPUS = '1'; $env:INSTA_UPDATE = 'always'; cargo nextest run -p warden-jarmeta --run-ignored only -E 'test(rede_corpus)'` baixa os jars (cache em `target/tmp/jarmeta-corpus`, sha1 conferido), refaz `jars/` e os dourados.
3. Revise o diff dos dourados antes de commitar.

Sem as variáveis, `cargo xtask test-network` confere que a extração versionada continua idêntica aos jars reais e que o modelo bate com os dourados.

## Licenças

As licenças de cada projeto estão no `corpus.json` (para a CurseForge, que não expõe a licença na API, "ver página do projeto"). Os arquivos copiados são descritores de metadados, guardados só para teste num repositório privado (ADR-0004).
