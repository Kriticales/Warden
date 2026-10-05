# ADR-0050 — Instância pronta para os jogadores no Prism Launcher

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão do dono (R7, decisão 1, aprovada em 04/10/2026) + decisão técnica (tarefa D7: formato)

## Contexto

Pelo caminho da [ADR-0028](0028-publicacao-no-github.md), o jogador precisa de 4 passos no Prism Launcher (criar a instância, baixar o bootstrap, colar o comando antes de iniciar, abrir o jogo). A pesquisa R7 (`docs/research/08-concorrencia-e-mercado.md` §8.1, lacuna L1) mostrou que é aí que um amigo leigo desiste. O dono aprovou na v1 um arquivo que o jogador **só arrasta para o Prism**, e que **o Prism baixa os mods, inclusive os da CurseForge, com a chave dele**. Isso também reduz o risco R1 da R7 (a CDN da CurseForge passar a exigir chave, que o packwiz-installer não manda; ADR-0051).

O que o Prism importa de fato foi conferido no código do Prism Launcher 11.1.1 (release de 28/09/2026), em 04/10/2026:

- `launcher/InstanceImportTask.cpp`: um arquivo arrastado é reconhecido pelo conteúdo, pelo primeiro destes que aparecer: `modrinth.index.json` (`.mrpack`), `bin/modpack.jar` (Technic), `manifest.json` (zip da CurseForge) ou `instance.cfg` (instância do Prism/MultiMC). Arrastar arquivos para a lista de instâncias chama a mesma importação (`MainWindow::processURLs`).
- Instância do Prism/MultiMC (`instance.cfg` + `mmc-pack.json`): o Prism só extrai a pasta; **não baixa nenhum mod**. Num zip assim, quem baixaria os mods seria o packwiz-installer pelo comando antes de iniciar, sem chave da CurseForge.
- `.mrpack` (`modrinth/ModrinthInstanceCreationTask.cpp`): o Prism baixa cada arquivo pelas URLs de `files[].downloads`. Quando a importação vem de um arquivo local, ele mostra a lista "mods não confiáveis" (`UntrustedModsDialog`) para todo arquivo cujo endereço não seja `https://cdn.modrinth.com` e para jars soltos em `overrides/mods/`, e só continua se o jogador aceitar.
- `net/ApiHeaderProxy.h` e `buildconfig/BuildConfig.h`: todo pedido do Prism para `api.curseforge.com` **e para `edge.forgecdn.net`** leva o cabeçalho `x-api-key` com a chave do próprio Prism.
- Zip da CurseForge (`flame/FlameInstanceCreationTask.cpp`): o Prism resolve cada arquivo pela API da CurseForge com a chave dele, lê a memória recomendada (`minecraft.recommendedRam`) e mostra o diálogo de mods bloqueados para download manual; mods de fora da CurseForge só podem ir como jars embutidos em `overrides/`, que também caem na lista de "não confiáveis".

O `packwiz modrinth export` não serve como está: ele só põe no `modrinth.index.json` arquivos com URL de `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com` e `gitlab.com` (`modrinth/export.go`, `whitelistedHosts`), e embute como jar em `overrides/` todo mod da CurseForge (modo `metadata:curseforge`), mesmo com `--restrictDomains=false`.

## Decisão

- **Formato:** um `.mrpack` gerado pelo próprio Warden (`warden-export::prism`), chamado na interface de **"Instância pronta para o Prism"** e salvo como `<pack>-<versão>-prism.mrpack`. Conteúdo:
  - `modrinth.index.json` com `game`, `versionId`, `name`, `summary` e `dependencies` (Minecraft e loader com a versão exata do `pack.toml`);
  - mods, resource packs e shaders do **Modrinth** pela URL de `cdn.modrinth.com` do metafile;
  - mods da **CurseForge** pela URL da CDN da CurseForge (`https://edge.forgecdn.net/files/<id ÷ 1000>/<id mod 1000>/<arquivo>`), montada a partir do ID do arquivo e do nome que já estão no `.pw.toml`, sem gravar nenhuma resposta da API (ARCHITECTURE §17); antes de gerar, o Warden confere na API, só em memória, que cada arquivo permite distribuição por terceiros; o Prism baixa esses arquivos com a chave dele;
  - arquivos de **link direto** pela URL do metafile;
  - `hashes.sha1`, `hashes.sha512` e `fileSize` de cada arquivo, tirados do cache de downloads (o gancho 1 da ARCHITECTURE §21.7 guarda os quatro hashes; o que faltar é baixado antes, com progresso);
  - `env` pelo lado do metafile (só valores da especificação, nunca `unknown`) e opcionais como `optional`;
  - `overrides/` com os arquivos do índice que não são referência (configs, scripts, arquivos locais), pelo mesmo filtro da exportação (T19); `server-overrides/` nunca entra.
- **Mods que impedem gerar:** mod da CurseForge com distribuição bloqueada (o Prism não teria como baixar): o Warden para e oferece **Trocar pelo Modrinth** quando o mesmo arquivo existe lá, como no `.mrpack` comum (T19). Arquivo local de terceiros entra em `overrides/` só com a confirmação de licença.
- **Onde fica (sem abas, ESTRUTURA §13):** na seção **Exportar**, como formato "Instância pronta para o Prism (.mrpack)"; e no resultado do **Publicar versão**, ao lado de "Como os jogadores instalam", com o botão **Baixar instância pronta para o Prism**. Ao publicar, o arquivo também vai anexado à GitHub Release da versão, para o jogador baixar pela página da Release.
- **O que o jogador vê:** o passo a passo diz "arraste o arquivo para a janela do Prism" e avisa que o Prism vai listar os mods da CurseForge (e os links diretos) como "não confiáveis" e pedir confirmação, porque vêm de fora do Modrinth.
- **Prioridade:** P1 da v1 (decisão do dono), tarefa E-04 do ROADMAP.

## Alternativas consideradas

- **Instância do Prism/MultiMC com o packwiz-installer-bootstrap** (proposta original da R7 §8.1: `instance.cfg` com o comando antes de iniciar, `mmc-pack.json` e o bootstrap): atualiza sozinha a cada versão publicada e leva a memória recomendada, mas o Prism não baixa nada: quem baixa é o packwiz-installer, sem chave da CurseForge (o risco R1 continua). Contraria o "o Prism baixa os mods com a chave dele" da decisão do dono. O caminho do bootstrap continua existindo no passo a passo da T18 para quem quer atualização automática.
- **Zip da CurseForge (`manifest.json`):** o Prism resolve tudo pela API com a chave dele e lê a memória recomendada, mas os mods do Modrinth (a maioria, porque a busca combinada prefere o Modrinth) teriam de ir como jars embutidos, com licença a confirmar mod a mod e a lista de "não confiáveis" para todos eles.
- **`.mrpack` do `packwiz modrinth export`:** embute os mods da CurseForge como jars em `overrides/` (verificado no código), que é justamente o que a decisão quer evitar.
- **Um zip com `modrinth.index.json` e `instance.cfg` juntos:** o Prism para no primeiro arquivo reconhecido na ordem do zip; frágil.

## Consequências

- Cada versão publicada gera um arquivo novo: a instância pronta **não se atualiza sozinha**. Para receber a versão seguinte, o jogador arrasta o arquivo novo (o Prism cria outra instância) ou usa o caminho do bootstrap. O resultado do Publicar versão explica as duas opções em linguagem simples.
- O `.mrpack` não tem campo de memória: a memória recomendada do pack aparece no passo a passo ("No Prism, em Editar instância → Configurações → Memória, use 6 GB").
- A conferência de hash fica com o Prism (ele confere `sha1`/`sha512` de cada download).
- A URL da CDN da CurseForge é montada pelo Warden: se a CurseForge mudar o formato do endereço, a geração precisa mudar junto. A E-04 confere, num teste de rede, que a URL montada responde para arquivos reais, e confere nos termos da API da CurseForge que gravar o endereço da CDN num arquivo para o jogador é permitido; se não for, a decisão volta ao dono com a alternativa do zip da CurseForge.
- A importação real no Prism não roda na CI: a E-04 valida o arquivo contra as regras lidas do código do Prism e o roteiro do marco importa o arquivo no Prism 11.x no Windows.
