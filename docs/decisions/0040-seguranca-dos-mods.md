# ADR-0040 — Checagem de segurança dos mods

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** decisão do dono + técnica (tarefa D5, decisão D27)

## Contexto

Um mod é código Java que roda com as permissões de quem abre o jogo. Casos reais:

- **fractureiser** (2023): contas de autores da CurseForge foram roubadas e arquivos oficiais receberam um estágio 0 que criava um `URLClassLoader` por reflexão, montava os textos com `new String(byte[])` e baixava o resto de um IP fixo (<https://github.com/fractureiser-investigation/fractureiser>).
- **Windows Borderless** (Modrinth, maio de 2024): um projeto publicado no próprio Modrinth (<https://modrinth.com/news/article/windows-borderless-malware-disclosure/>).
- **Stargazers Ghost Network** (2025): mods falsos distribuídos pelo GitHub (<https://research.checkpoint.com/2025/minecraft-mod-malware-stargazers/>); **WeedHack** e **Myth Stealer** (2026), também fora das plataformas.

O Modrinth tem um scanner interno (Delphi, código fechado) e a API não expõe o resultado; a CurseForge diz fazer dezenas de testes automáticos e revisão manual (<https://blog.curseforge.com/safeguarding-our-community-curseforge-fighting-malware-incident-report/>). Ferramentas da comunidade estudadas: nekodetector (MIT, parado desde 2023), jar-infection-scanner da Overwolf (MIT, C#), jNeedle (MIT, Kotlin, banco remoto de "needles"), Concoction da MMPA (MIT, modelos JSON com níveis de risco), jSus (heurística, muitos falsos positivos), modrinth/oracle (Rust, só lista de hashes, arquivado) e jarspect (Rust, usa `cafebabe`, veredito por LLM).

## Decisão

Nenhum jar é executado; os arquivos só são lidos. Duas partes:

- **Conferência com o arquivo oficial.** Modrinth: `POST /v2/version_files` com o sha512, que tem de pertencer ao projeto e à versão do `.pw.toml`. CurseForge: `POST /v1/fingerprints` com o murmur2 de 32 bits (seed 1, sem os bytes 9, 10, 13 e 32), confirmado pelo SHA-1 de `file.hashes`, porque o murmur2 sozinho pode colidir; `fileStatus = 6` (MalwareDetected) vira erro. **Só hashes saem do computador.**
- **Busca estática de sinais**, na crate nova `warden-security`, com `cafebabe` (0BSD, já escolhido para o raio-x, [ADR-0034](0034-analise-estatica-do-pack.md)) e `zip`, descendo nos jars embutidos. Os **sinais conhecidos** (lista da [ADR-0041](0041-lista-de-sinais-embutida.md)) são procurados em todos os jars. Os **pontos de atenção** (heurística: carregar código da internet, textos montados em bytes, executar programas, webhooks, IPs fixos, caminhos de dados de navegador e de launcher) só nos jars que não conferem com nenhum oficial: mods legítimos como o Sodium usam `Runtime.exec`, e bibliotecas embutidas disparam alertas, como relatou quem escaneou o Modrinth inteiro.
- **Obrigatória antes de publicar.** Um erro de segurança tira o "Testar mesmo assim" do teste e bloqueia a publicação.
- **Confiar neste arquivo** grava a confiança por hash em `.warden/trust.toml`, depois de o usuário digitar o nome do mod; outro arquivo volta a ser verificado.
- **Honestidade:** a interface diz que o Warden "não é um antivírus".

## Alternativas consideradas

- Integrar antivírus ou VirusTotal: envia arquivos a terceiros.
- YARA-X: dependência grande para o ganho atual; pode ser avaliado depois.
- Veredito por LLM (como o jarspect): não determinístico, não dá para testar.
- Procurar rastros do malware no computador: fora do papel do Warden; vira P2.
- Usar uma ferramenta pronta como dependência: as opções estão paradas, em outra linguagem ou dependem de banco remoto.

## Consequências

- Limitações aceitas e ditas na tela: payload baixado em tempo de execução, ofuscação e sinais que envelhecem passam sem ser notados. "Oficial" não quer dizer "limpo" (fractureiser e Windows Borderless eram arquivos oficiais); por isso os sinais conhecidos valem também para eles.
- A conferência usa os hashes que o índice do cache já guarda desde a v1 ([ADR-0039](0039-warden-1-1-profissional.md), gancho 1). Testes com jars sintéticos; nenhum malware real entra no repositório.
- SPEC T28.
