# ADR-0028 — Publicar versões no GitHub para distribuir o pack

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono · **Substitui em parte:** ADR-0015 (item GitHub) e a decisão D2 da SPEC ("um repositório privado por pack")

## Contexto

A ADR-0015 tratava o GitHub como cópia do histórico, num repositório sempre privado. O dono decidiu que o GitHub serve para **distribuir o pack aos jogadores**, que atualizam sozinhos pelo link do `pack.toml` com o packwiz-installer-bootstrap (`https://raw.githubusercontent.com/<dono>/<repo>/main/pack.toml`). Para isso o repositório precisa ser público, e os jogadores só podem receber o que o dono liberou.

## Decisão

- **Salvar versão continua local.** Uma versão salva pode ser marcada como **versão final** (pronta para os jogadores), no diálogo de salvar ou no Histórico; só versões finais podem ser publicadas.
- **Publicar versão** (nome na interface; substitui "Enviar ao GitHub") envia ao GitHub **só o conteúdo do pack** daquela versão: o mesmo conjunto da exportação (`pack.toml`, `index.toml` e os arquivos do índice), mais `CHANGELOG.md` (notas de todas as versões publicadas) e `.gitattributes` com `* -text`. Nunca `.warden/`, o histórico de trabalho, versões não publicadas, mundos ou registros.
- Cada publicação é um commit numa **linha de publicação** própria (local `refs/warden/publish/main`, remota `main`), com tag `vX.Y.Z` e uma **GitHub Release** com as notas. As notas comparam com a **última versão publicada**: mods adicionados, removidos e atualizados (com versões), resource packs e shaders, configs alteradas e seção "Atenção" quando houver remoção perigosa.
- **Visibilidade:** na primeira publicação o Warden explica que o link só funciona para os jogadores em repositório **público** e recomenda público; **privado** fica como opção "só backup, sem atualização automática para jogadores". Um repositório por pack.
- **Antes de publicar**, avisos: mods da CurseForge com distribuição bloqueada (o jogador não consegue baixar sozinho; sugerir o Modrinth quando o mesmo arquivo existir lá), versão não testada depois da última mudança, problemas pendentes. Bloqueia de verdade: qualquer coisa com cara de chave ou token no conteúdo a publicar, e arquivos que a higiene reprova.
- **Depois de publicar:** link do `pack.toml` com "Copiar link", passo a passo curto para jogadores (Prism Launcher/MultiMC com o packwiz-installer-bootstrap) e "Copiar texto das notas da versão".

## Alternativas consideradas

- Enviar a branch de trabalho inteira (ADR-0015): publicaria `.warden/` e versões não liberadas, e os jogadores receberiam qualquer versão salva.
- Branch de publicação dentro da mesma linha de histórico: misturaria commits de trabalho e de publicação.
- Repositório sempre privado (D2 antiga): o link do `pack.toml` não funciona para os jogadores.

## Consequências

- `warden-versioning` ganha versão final, linha de publicação e Release pela API (ARCHITECTURE §11); a exportação a partir de uma versão salva passa a ser P0, porque a publicação depende dela (ROADMAP E-01, V-03).
- O token do GitHub precisa poder criar repositórios públicos, enviar conteúdo e criar Releases (ROADMAP §2).
- O resto da ADR-0015 continua (git embutido, versões locais, pontos de segurança, voltar versão).
