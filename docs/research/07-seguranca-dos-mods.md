# Pesquisa 07 — Segurança dos mods: antivírus, consulta por hash e ferramentas especializadas

- **Tarefa:** R6 (onda 0) · **Data:** 2026-10-04 · **Branch:** `Kriticales/research-r6-seguranca-dos-mods`
- **Código de teste:** `spikes/r6/` (`probe/`, `yarax/`, `hashlookup/`, `fixtures/iocs-publicos.txt`)
- **Máquina:** Windows 11 Pro 26300, Kaspersky Premium 21.26 (bases de 04/10/2026) ativo, Defender desligado ("Not running").
- **Marcas:** **[verificado]** = executei e vi o resultado; **[código]** = li o código-fonte; **[inferência]** = conclusão minha, sem teste.

## Resumo em linguagem simples

1. **O antivírus do computador funciona com o Warden, sem enviar nada para a internet.** O Windows tem uma porta oficial (a AMSI) pela qual um programa entrega bytes ao antivírus instalado e recebe "perigoso" ou "não detectado". Testei com o Kaspersky: ele reconheceu o arquivo de teste EICAR e também o padrão sintético do fractureiser, inclusive escondido num jar dentro de outro jar, desde que o Warden entregue a ele cada jar interno separado. Nos 20 mods reais testados, nenhum alarme falso. **[verificado]**
2. **Quando o antivírus acha algo, ele avisa o usuário por conta própria** (balão do Kaspersky e registro nos relatórios dele). Pela AMSI ele não apaga nada, porque só recebeu bytes. Quando o arquivo perigoso está gravado no disco, a proteção em tempo real bloqueia o arquivo na hora e depois o apaga, guardando uma cópia no backup do Kaspersky. **[verificado]**
3. **As ferramentas especializadas em Minecraft são pequenas e quase todas paradas.** As melhores têm entre 3 e 17 padrões, a maioria do fractureiser (2023). Várias têm licença MIT, então o Warden pode **copiar os padrões com atribuição** para a própria lista. Nos testes, nenhuma delas pegou o padrão do fractureiser escondido num jar embutido. **[verificado]**
4. **A consulta por hash na internet ajuda pouco com mods.** De 20 hashes públicos de mods maliciosos (fractureiser, Windows Borderless, Stargazers), os serviços sem chave conheciam no máximo 3 ou 2. E onde conheciam, só 22% a 34% dos antivírus detectavam o arquivo. O VirusTotal deve ser melhor, mas exige uma chave (conta grátis) e aceita só 4 consultas por minuto. **[verificado]** (VirusTotal: **[inferência]**)
5. **O YARA-X não compensa agora.** Ele funciona e é rápido, mas aumenta o executável em 23 MB e traz 232 bibliotecas a mais. E quase não existem regras YARA públicas para mods. A busca própria do Warden faz o mesmo trabalho. **[verificado]**

**Recomendação:** quatro camadas. (1) Conferência com o arquivo oficial e (2) busca de sinais própria continuam ligadas sempre, como na ADR-0040, agora com mais padrões de terceiros. (3) **Antivírus do computador pela AMSI, ligado por padrão no Windows.** (4) **Consulta de reputação por hash, opcional**: desligada por padrão, só para arquivos de fora das plataformas, com a chave do VirusTotal do usuário. YARA-X fica de fora. Os detalhes estão na §7, e as decisões para o dono, na §8.3.

---

## 1. Contexto e o que a ADR-0040 disse

A ADR-0040 descartou "integrar antivírus ou VirusTotal" porque isso "envia arquivos a terceiros". O motivo está **impreciso** nos dois casos:

- **AMSI:** os bytes vão para o antivírus **local**, no mesmo computador. Nada sai pela rede por conta do Warden. O próprio antivírus pode mandar amostras à nuvem dele (KSN no Kaspersky, MAPS no Defender), conforme a configuração que o usuário já escolheu para todos os programas. **[verificado: chamada local; o envio à nuvem do antivírus é [inferência] pela documentação dos fornecedores]**
- **VirusTotal e afins:** a consulta **por hash** só manda a impressão digital (64 caracteres hexadecimais). O arquivo só seria enviado se o Warden fizesse upload, e isso não está proposto. **[verificado: as chamadas testadas mandam só o hash]**

O dono quer a ferramenta definitiva e prefere cobertura a economia. Por isso esta pesquisa testou cada opção na prática.

## 2. Ferramentas especializadas em malware de Minecraft

### 2.1 Comparativo

| Ferramenta | Licença | Manutenção (último push / release) | Formato dos padrões | Nº de padrões | Reaproveitável pelo Warden? | Detecção nos testes |
|---|---|---|---|---|---|---|
| **nekodetector** (MCRcortex) | MIT | 06/2023 / 1.1-pre | Sequências de instruções ASM em Java: SIG1 (estágio 0), SIG2 (`Runtime.exec` + Base64), SIG3 (bytes do IP) | 3 | **Sim**, com atribuição (MIT) **[código]** | Não leu os 2 jars simples, que o Kaspersky tinha bloqueado; não pegou os 2 embutidos, porque não desce em jars internos **[verificado + código]** |
| **jNeedle** (KosmX) | MIT | 04/2024 / 1.1.0-local (07/2023); banco remoto `maven.kosmx.dev/dev/kosmx/needles` de 22/08/2023 | DSL Kotlin → arquivos `.jasm` (sequência de instruções ou constante `ldc`) | 11 (fractureiser ×3, SessionLogger ×2, Skyrage ×4, WeirdUtils, YoinkRat) | **Sim**, com atribuição (MIT) **[código]** | 0 achados; não abriu os jars embutidos sintéticos (erro do leitor `lljzip`: "No Central-Directory-File-Header found!"); nos mods reais leu 204 jars embutidos sem erro e sem falso positivo **[verificado]** |
| **jar-infection-scanner** (Overwolf/CurseForge) | MIT | 08/2023 | 3 sequências de bytes cruas do bytecode (IP do estágio 0, `files.skyrage.de`, `-jar`) | 3 | **Sim** (MIT); usei o padrão do IP no teste YARA **[código]** | Não executado (C#/WinForms) |
| **MCAntiMalware** (OpticFusion1) | GPL-3.0 (código); banco **sem licença** | 05/2026 (só atualização de dependências) / 15.16 (09/2025) | Classes Java de "checks" + SQLite remoto (`database.db`) | 281 famílias, todas **Spigot** (plugins de servidor); 349 SHA-1 de jars, 567 textos, 169 caminhos de classe, 49 URLs | **Não**: código GPL-3.0 e banco sem licença (todos os direitos reservados); foco em plugins de servidor **[verificado no banco]** | 0 achados nas amostras (nada de mods de cliente) **[verificado]** |
| **0xresetti/JarMalwareScanner** | MIT | 02/2026 | C#: 10 "variantes" (Weedhack, AdamRAT, STRRAT, Microstealer, DonutSMP...) por combinação de nomes de classe e textos + `KnownHashes.json` com **71 SHA-256 de classes** maliciosas | 10 + 71 hashes | **Sim** (MIT): os hashes de classe e as combinações viram sinais **[código]** | Não executado (C#) |
| **Minecraft-Malware-Scanner** (Mackery6969) | MIT | criado em 01/10/2026 | `signatures.json`: regras com condições `string`/`regex`/`ref`/`path` sobre o pool de constantes e textos de `byte[]` | 17 (fractureiser ×6, Skyrage, roubo de sessão, persistência...) | **Sim** (MIT), mas é um projeto de 3 dias; serve de referência de formato **[código]** | Não executado (Python) |
| **jarspect** (Microck) | Apache-2.0 | 09/2026 | 6 regras YARA específicas de campanhas + MalwareBazaar + veredito por LLM | 6 | Os textos das regras sim (Apache-2.0, com aviso); o veredito por LLM não (ADR-0040) **[código]** | Regras aplicadas pelo YARA-X nos 20 mods reais: 0 falsos positivos **[verificado]** |
| **Concoction** (MMPA) | MIT | 08/2023 | Modelos JSON com níveis de risco | — | Já é a inspiração do formato (ADR-0041) | — |
| **fractureiser-web-detector** (douira) | MIT | 09/2023 | `signatures.ts` (sequências de bytecode) | poucos | Referência | — |
| **modrinth/oracle** | GPL-3.0, arquivado | 05/2024 | Lista de hashes | — | Não | — |
| **jSus** (Toshayo) | 0BSD | 09/2026 | Heurística ("gerador de falsos positivos", nas palavras do autor) | — | Não | — |

Fontes: <https://github.com/MCRcortex/nekodetector>, <https://github.com/KosmX/jneedle>, <https://maven.kosmx.dev/dev/kosmx/needles/files.json>, <https://github.com/overwolf/jar-infection-scanner>, <https://github.com/OpticFusion1/MCAntiMalware>, <https://github.com/OpticFusion1/MCAntiMalwareDatabase>, <https://github.com/0xresetti/JarMalwareScanner>, <https://github.com/Mackery6969/Minecraft-Malware-Scanner>, <https://github.com/Microck/jarspect>, <https://github.com/Minecraft-Malware-Prevention-Alliance/concoction>, <https://github.com/douira/fractureiser-web-detector>, <https://github.com/modrinth/oracle>, <https://github.com/Toshayo/jSus>. Metadados (licença, último push, estrelas) conferidos com `gh api repos/<repo>` em 04/10/2026 **[verificado]**.

**Cuidado registrado:** o repositório `MCAntiMalwareDatabase` guarda **malware real** na pasta `malware/` (o README diz "Working malware can be found in the malware directory"). Baixei só o `database.db` (SQLite, 348 KB), nunca a pasta. O Warden não deve clonar esse repositório nem apontar para ele. **[verificado]**

### 2.2 O que as plataformas e a comunidade usaram

- **fractureiser (06/2023, CurseForge e Bukkit):** a CurseForge publicou o jar-infection-scanner da Overwolf e a comunidade usou o nekodetector. Os padrões e os IOCs estão em <https://github.com/fractureiser-investigation/fractureiser> (docs/tech.md), com os IPs `85.217.144.130` e `107.189.3.101` e os SHA-1 dos estágios 1 a 3. O Modrinth disse ter escaneado todos os projetos e não ter sido afetado (<https://x.com/modrinth/status/1666853947804463115>). **[código/fonte]**
- **Windows Borderless (05/2024, Modrinth):** um usuário denunciou o mod depois de ter a conta do Discord roubada. A equipe do Modrinth decompilou o mod e o removeu em minutos. O Modrinth publicou 5 SHA-1 e prometeu "a shared system to effectively quarantine known malicious mods by creating a web API" para launchers (<https://modrinth.com/news/article/windows-borderless-malware-disclosure/>). Não achei essa API publicada até hoje. **[fonte; a ausência da API é [inferência] pela busca]**
- **Stargazers Ghost Network (2025, GitHub):** segundo a Check Point, "the Java downloader is undetected by all antivirus engines across VirusTotal" (<https://research.checkpoint.com/2025/minecraft-mod-malware-stargazers/>). A empresa publicou 15 SHA-256. **[fonte]**
- **2026:** *Myth Stealer* num falso "Lithium Extras" (<https://gbhackers.com/fake-minecraft-mod-drops/>) e *Weedhack*, que tem padrão no scanner do 0xresetti. Também aparecem serviços web de "RAT scanner" que exigem **enviar o jar**, o que o Warden não fará. **[fonte]**
- Um terceiro escaneou o Modrinth inteiro (40.146 varreduras, 140.382 achados) e concluiu que heurística ampla gera muito ruído (<https://www.ruse.tech/blogs/scanning-every-mod-on-modrinth>). Isso confirma a regra da ADR-0040: pontos de atenção só em arquivos de fora das plataformas. **[fonte]**

### 2.3 Qualidade da detecção

Todas usam o mesmo princípio: sequências de instruções ou constantes de casos **conhecidos**. Nenhuma pega um caso novo. A diferença entre elas está no cuidado com jars embutidos e no tamanho da lista. O protótipo do Warden (`spikes/r6/probe/src/sigscan.rs`, com `cafebabe`) aplicou o SIG1 do nekodetector/jNeedle, os IOCs de texto e os pontos de atenção:

- Detectou o estágio 0 nas 4 variantes (IP real de 2023 e endereço de documentação 192.0.2.1, com e sem jar embutido), com classe e método na evidência. **[verificado]**
- Não deu nenhum sinal conhecido nos 20 mods reais (74 MB, 49.087 entradas, 2,8 s). Deu 3 pontos de atenção `Runtime.exec`, 2 no Sodium e 1 no Xaero's Minimap, que seriam ruído se fossem aplicados a arquivos oficiais. Isso confirma a regra da ADR-0040. **[verificado]**

**Conclusão:** o Warden deve **juntar os padrões de terceiros** com licença MIT na própria `signatures.toml`, com atribuição em `THIRD_PARTY.md`. São 3 do nekodetector, 11 do jNeedle, 3 da Overwolf, 71 hashes de classe e 10 combinações do 0xresetti, além das regras do Mackery6969 que fizerem sentido. Usar qualquer dessas ferramentas como dependência não compensa: todas estão em outra linguagem, paradas ou imaturas. A descida em jars embutidos é obrigatória e é justamente o ponto onde as ferramentas prontas falharam no teste.

## 3. Antivírus do computador pela AMSI

### 3.1 Como foi testado

`spikes/r6/probe/src/amsi.rs` chama `AmsiInitialize` → `AmsiOpenSession` → `AmsiScanBuffer` (crate `windows-sys` 0.61, `Win32_System_Antimalware`), entregando bytes que estão **na memória**, sem gravar nada em disco. O provedor AMSI registrado na máquina é o do Kaspersky: `HKLM\SOFTWARE\Microsoft\AMSI\Providers\{3C92AB06-BA68-4DE2-87CA-96570B547BCA}` → `...\Kaspersky 21.26\x64\com_antivirus.dll`. Pelo `avp.com STATUS`, a tarefa "AMSI" aparece como `running`. **[verificado]**

### 3.2 Resultados (comando `r6probe amsi-mem`)

| Amostra | Jar inteiro | Cada entrada (descendo nos embutidos) |
|---|---|---|
| `eicar.com` (68 bytes) | **DETECTED (32768)** | — |
| EICAR dentro de jar (com e sem compressão) | NOT_DETECTED | `eicar.com` **DETECTED** |
| EICAR em jar embutido em outro jar | NOT_DETECTED | jar interno: NOT_DETECTED; `inner.jar!/eicar.com` **DETECTED** |
| EICAR como texto dentro de uma `.class` | NOT_DETECTED | NOT_DETECTED (esperado: o EICAR só vale no começo do arquivo) |
| Estágio 0 sintético, IP de 2023 | **DETECTED** | a `.class` **DETECTED** |
| Estágio 0 sintético, IP 192.0.2.1 (TEST-NET) | **DETECTED** | a `.class` **DETECTED** |
| Estágio 0 em jar embutido (as duas variantes) | NOT_DETECTED | jar interno **DETECTED**; a `.class` **DETECTED** |
| Pontos de atenção (webhook, `Runtime.exec`, IP) | NOT_DETECTED | NOT_DETECTED |
| Limpo e limpo embutido | NOT_DETECTED | NOT_DETECTED |

**[verificado]**. Repeti duas vezes, com três nomes de conteúdo diferentes (o nome real, `arquivo.bin` e vazio), e o resultado foi idêntico (`r6probe amsi-nomes`): é **determinístico e não depende do nome**. **[verificado]**

O que os resultados mostram **[verificado + inferência]**:
- O Kaspersky abre um nível de jar quando recebe o buffer, porque detectou o jar inteiro com o estágio 0. Mas **não abre o jar embutido** que está dentro dele. A detecção é **heurística e estrutural**: a variante com o endereço TEST-NET, que nunca existiu em malware, também foi detectada. O nome que a linha de comando mostrou foi `HEUR:Trojan.Java.Generic`.
- O EICAR dentro de um jar só aparece quando a entrada é entregue sozinha. Pela AMSI, o Kaspersky não aplica a assinatura EICAR dentro de zips.
- **Estratégia boa e barata:** entregar o **jar inteiro e cada jar embutido** como buffers separados. Nos testes, isso pegou todas as variantes do estágio 0. Entregar cada entrada pega também o EICAR em jar, mas custa caro (§3.4).

### 3.3 O que o Kaspersky fez e o que o usuário viu

Contei pelos relatórios exportados com `avp.com REPORT AMSI /RA:` (`spikes/r6/hashlookup/contar-eventos-kaspersky.py`) e por duas capturas de tela. As capturas foram apagadas porque mostravam outras janelas do dono.

| Caminho | Detecções no teste | O que o Kaspersky fez | O que o usuário viu |
|---|---|---|---|
| **AMSI** (bytes em memória) | **30** (12 no `amsi-mem` + 18 no `amsi-nomes`) | Nada com arquivos (não existe arquivo). Devolveu `DETECTED` ao Warden e registrou em Relatórios → **Proteção AMSI**: "Objeto malicioso detectado", tipo "Cavalo de Troia", aplicativo `r6probe.exe`, e "O resultado da verificação do objeto foi enviada para um aplicativo de terceiros" | **Um balão do Kaspersky** com "O resultado da verificação do objeto foi enviada para um aplicativo de terceiros" (visto na tela) |
| **Proteção em tempo real** (amostras gravadas em disco, 3 rodadas) | **9** (`eicar.com` e os 2 jars simples do estágio 0, em cada rodada) | Bloqueou o arquivo na hora (ao ler, `Acesso negado (os error 5)` 5 s depois) e depois **excluiu**, criando "cópia de backup". Na 1ª e na 3ª rodada a exclusão veio de 60 a 90 s depois; na 2ª, em segundos. Os jars com EICAR dentro e os jars **embutidos** com o estágio 0 **não foram tocados**: o monitor em tempo real não abre arquivos compactados | Balões "Objeto excluído: C:\…\stage0-testnet.jar" (o último ficou no histórico de notificações do Windows) |
| **`avp.com SCAN`** (linha de comando) | 1ª: 5 objetos; 2ª: 5 objetos; 3ª (20 mods reais): 0 | 1ª, com as opções perdidas (abaixo): **apagou o conteúdo de dentro de 5 jars** (os jars de EICAR viraram zips vazios de 22 bytes), com cópia no backup. 2ª, com `/i0`: só relatou | Evento "A verificação foi concluída. Foram encontradas ameaças" |

**Total observado: 49 detecções**, sendo 30 pela AMSI, 9 em tempo real e 10 pelo `avp.com`. O número exato de balões não pôde ser medido: o histórico de notificações do Windows (`wpndatabase.db`, lido numa cópia) guarda só os 2 últimos do Kaspersky. Pelo relato do orquestrador, o Kaspersky "apitou sem parar". Ficaram no backup do Kaspersky cópias de `eicar.com` (3), dos jars do estágio 0 (6) e dos 5 jars alterados pela 1ª verificação. O dono pode apagá-las em Kaspersky → Backup e restauração; não mexi nelas. **[verificado]**

**Consequência para o produto:** o antivírus avisa **por conta própria** a cada detecção, mesmo pela AMSI. Por isso:
- O Warden **não pode** usar o EICAR como "teste de vida" da AMSI a cada verificação, porque isso faria o antivírus apitar à toa. A presença do antivírus sai do registro (`AMSI\Providers`) e da Central de Segurança (`root/SecurityCenter2`, `AntiVirusProduct.productState`; aqui Kaspersky `0x041000` = ligado e atualizado, Defender `0x060100` = desligado). **[verificado]**
- Com mods reais, a AMSI **não deu nenhum alarme**: 49.087 entradas de 20 mods populares, 0 detecções, 0 balões. Um aviso do antivírus durante a checagem do Warden é, portanto, sinal sério, e o Warden deve explicar ao usuário que o aviso que ele viu veio dessa checagem. **[verificado]**
- A AMSI devolve só o código (perigoso ou não). **O nome da ameaça não vem**: ele aparece apenas nos relatórios do próprio antivírus. **[verificado]**

### 3.4 Desempenho (20 mods reais, 74 MB, comando `r6probe amsi-bench`)

| Modo | Tempo total | Observação |
|---|---|---|
| Jar inteiro | **0,95 s** | de 4,7 ms (TerraBlender) a 153 ms (Biomes O' Plenty, 21 MB). A primeira chamada da sessão custa de 0,2 a 0,5 s |
| Cada entrada (49.087) | **136 s** (≈ 2,8 ms por entrada) | Create sozinho: 25.712 entradas em 74 s |
| `avp.com SCAN /i0 /fa` na pasta | **140 s** (42.881 objetos) | Mais de 1 s de custo fixo por chamada |

**[verificado]**. Conclusão: entregar o jar inteiro e cada jar embutido cabe no orçamento da CA-T28-07. Entregar cada entrada fica restrito aos arquivos **de fora das plataformas**, que são poucos.

### 3.5 Linha de comando do antivírus

- **Kaspersky:** `avp.com SCAN <pasta> /i0 /fa /RA:<relatório>` funciona sem instalar nada, só relata, devolve saída 3 com detecção e 0 sem detecção e dá o nome da ameaça no relatório. **Armadilha grave:** pelo Git Bash, `/i0` e `/fa` viraram caminhos (`C:/Program Files/Git/i0`). O Kaspersky ignorou as opções, usou o padrão "Perguntar depois da verificação" e **apagou conteúdo dos jars**. Pelo PowerShell, o mesmo comando só relatou. **[verificado]**
- **Defender:** `MpCmdRun.exe -Scan -ScanType 3 -File <jar> -DisableRemediation` falhou com `0x80004005` nesta máquina, porque o Defender não está rodando. **[verificado]** Onde o Defender é o antivírus ativo, o comando funciona e é documentado pela Microsoft. **[inferência]**
- **Recomendação:** não usar linha de comando. Cada fornecedor tem a sua, com opções e códigos diferentes, e um erro de opção pode apagar arquivos do usuário. Além disso, a linha de comando é mais lenta que a AMSI.

### 3.6 Funciona com o Defender em outro computador?

O Defender é o provedor AMSI padrão do Windows 10/11 e responde a `AmsiScanBuffer` de qualquer programa: ferramentas de segurança usam a AMSI exatamente assim, e a Microsoft documenta a interface para "qualquer aplicativo". Antivírus de terceiros que se registram como provedores AMSI (Kaspersky, ESET, Bitdefender, Avast e outros) funcionam do mesmo jeito. **[inferência pela documentação: <https://learn.microsoft.com/windows/win32/amsi/antimalware-scan-interface-portal>]** O Kaspersky foi o único testado. **Não verifiquei** o que `AmsiScanBuffer` devolve quando **nenhum** provedor está registrado; por isso o Warden deve conferir o registro antes (§7). A CI do GitHub em Windows tem o Defender ativo, então um teste de integração com o EICAR **em memória** deve ser viável lá. **[inferência]**

Outros pontos: o Kaspersky pôs o `r6probe.exe`, que não é assinado, no grupo "restrito, baixa restrição" do controle de aplicativos, e a AMSI funcionou do mesmo jeito. **[verificado]** No Linux não há AMSI: a camada fica indisponível e a tela diz isso.

## 4. Consulta por hash, sem enviar arquivo

### 4.1 Serviços

| Serviço | Chave? | Plano grátis | Termos (uso pessoal) | O que devolve | Tempo medido | Testado sem chave |
|---|---|---|---|---|---|---|
| **VirusTotal** (v3 `GET /files/{hash}`) | **Sim** (conta grátis) | 4 req/min, 500/dia | API pública "must not be used in commercial products or services" e "must not be used in business workflows that do not contribute new files"; uso pessoal ok | Veredito de cerca de 70 motores, nomes das ameaças, primeira e última vez visto | — | `401 AuthenticationRequiredError` em 0,6 s **[verificado]** |
| **MalwareBazaar** (abuse.ch) | **Sim** (`Auth-Key`, grátis no portal da abuse.ch) | "fair use" | Grátis em fair use; uso comercial pode exigir plano pago | Só arquivos que estão no banco deles: família, tags, YARA, informações de fornecedores | — | `401 Unauthorized` em 2,2 s **[verificado]** |
| **YARAify** (abuse.ch, `lookup_hash`) | A documentação diz que sim | "fair use" | Igual à MalwareBazaar | Resultados de ClamAV e YARA, número de vezes visto | ~0,85 s | **Respondeu sem chave** (contraria a documentação; pode mudar) **[verificado]** |
| **MetaDefender Cloud** (OPSWAT) | **Sim** | Comunidade: 1.000 a 4.000 consultas de reputação por dia, conforme a página | "demo purposes and personal use" | Resultado por motor (cerca de 20 ou mais) | — | `401` em 1,1 s **[verificado]** |
| **Team Cymru MHR** (DNS `<hash>.hash.cymru.com` TXT) | **Não** | Sem cota no plano comunitário | "Free for non-commercial use" | Data em que foi visto pela última vez e **porcentagem de antivírus que detectam** | < 0,1 s (DNS) | Funciona; **pelo DNS só aceitou MD5 e SHA-1** (o SHA-256 do EICAR não respondeu, embora a página diga que aceita) **[verificado]** |
| **CIRCL hashlookup** | **Não** | best-effort | Grátis | Banco de **arquivos conhecidos e legítimos** (NSRL, distribuições); campo `KnownMalicious` às vezes | ~0,85 s | Funciona; o Sodium oficial não está lá (404). Serve pouco para mods **[verificado]** |
| **Kaspersky OpenTIP** | **Sim** (token de usuário registrado) | 2.000 req/dia | Uso pessoal permitido | Zona (vermelha/verde), detecções | — | `401` **[verificado]** |
| **Hybrid Analysis** | **Sim** | — | — | Relatórios de sandbox | — | redirecionou (`301`) **[verificado]** |

Fontes: <https://docs.virustotal.com/reference/public-vs-premium-api>, <https://bazaar.abuse.ch/api/>, <https://yaraify.abuse.ch/api/>, <https://metadefender.com/licensing>, <https://www.team-cymru.com/mhr>, <https://www.circl.lu/services/hashlookup/>, <https://opentip.kaspersky.com/Help/Doc_data/LimitationQuota.htm>.

### 4.2 Detecção real com IOCs públicos (só hashes; `spikes/r6/fixtures/iocs-publicos.txt`)

Comando: `python spikes/r6/hashlookup/consulta.py spikes/r6/fixtures/iocs-publicos.txt` **[verificado]**

| Hashes consultados | Team Cymru (sem chave) | YARAify (sem chave) | CIRCL | Modrinth `POST /v2/version_files` |
|---|---|---|---|---|
| EICAR (controle positivo) | SHA-1/MD5: visto, **100%** | visto 276 vezes, ClamAV e 4 regras YARA | conhecido (`KnownMalicious: malshare.com`) | — |
| Sodium 0.5.13 oficial (controle negativo) | não consta | não consta | não consta | (oficial) |
| fractureiser, 5 SHA-1 (estágios 1 a 3) | **3 de 5**: estágio 2 = **34%**, estágio 3 = **25%** e **22%**; estágio 1 e estágio 2 alternativo não constam | 0 (por SHA-1) | 0 | — |
| Windows Borderless, 4 SHA-1 | 0 de 4 | **2 de 4** (um só com `PUA.DiscordUrl` do ClamAV) | 0 | `{}`: o Modrinth **apagou** os arquivos e não os marca |
| Stargazers, 11 SHA-256 | (o DNS não aceita SHA-256) | 0 de 11 | 0 | — |

**Leitura:** os bancos sem chave conhecem pouco de malware de Minecraft. Mesmo onde conhecem, a maioria dos antivírus não detecta os arquivos: 22% a 34% para o fractureiser, três anos depois. A Check Point relata 0 motores do VirusTotal para o downloader Java do Stargazers. O VirusTotal deve conhecer mais desses hashes do que os serviços acima, mas isso **não foi verificado**, porque não criei conta. **[verificado / inferência]**

Outro achado útil: o Modrinth responde `{}` para os arquivos removidos do Windows Borderless. Na conferência com o oficial (W-02), um arquivo cujo `.pw.toml` diz "Modrinth" e cujo hash sumiu da plataforma deve virar "Não confere / arquivo removido da plataforma", e não "Não deu para conferir". **[verificado]**

### 4.3 Privacidade

- O hash de um mod público não revela nada além de "esta pessoa tem este mod". Para um jar particular (feito pelo próprio usuário), o hash não permite reconstruir o arquivo. Mas o serviço fica sabendo que alguém tem aquele arquivo e quando consultou. **[inferência]**
- **Pelo DNS, a Team Cymru recebe a consulta em texto aberto, e o provedor de internet também vê o hash.** É aceitável para SHA-1 de jars fora das plataformas, mas precisa constar na tela. **[inferência]**
- VirusTotal: a consulta por hash não envia o arquivo nem o torna público. O uso fica ligado à conta do usuário. **[inferência pelos termos]**
- Mandar só os arquivos **de fora das plataformas** reduz o vazamento ao mínimo: os mods oficiais já são conferidos com o Modrinth e a CurseForge.

### 4.4 Passo a passo para o dono (se escolher ligar o VirusTotal)

1. Abrir <https://www.virustotal.com/gui/join-us> e criar a conta grátis (e-mail e senha, ou conta Google).
2. Confirmar o e-mail.
3. Entrar, clicar no seu nome no canto superior direito → **API key** e copiar a chave.
4. No Warden (1.1): Configurações → Chaves → VirusTotal → colar. A chave fica no cofre do Windows, como as outras (ADR-0025).

MalwareBazaar (opcional): <https://auth.abuse.ch/> → entrar com GitHub, Google, X ou LinkedIn → **Optional: Auth-Key** → gerar → colar no Warden. A mesma chave vale para o YARAify.

## 5. YARA-X

| Item | Medida |
|---|---|
| Versão | `yara-x` 1.21.0 (BSD-3-Clause, VirusTotal), ativo (push em 02/10/2026) |
| Crates no grafo de dependências | **274**, contra 42 do mesmo spike sem YARA-X (+232; inclui `wasmtime`/`cranelift`) **[verificado com `cargo tree -e normal`]** |
| Executável de teste | **24,1 MB**, contra 0,9 MB sem YARA-X (+23 MB) **[verificado]** |
| Compilação limpa | 4 min 17 s; pasta `target` de 1,3 GB **[verificado]** |
| Compilação das regras | 7 a 11 ms **[verificado]** |
| Varredura | 20 mods (74 MB, 49.087 entradas, descompactando) em **1,0 a 1,3 s** **[verificado]** |
| Detecção nas amostras (em memória) | As 2 regras de teste do estágio 0 (bytes do IP, da Overwolf, e descritores no pool) pegaram as 4 variantes, inclusive as embutidas; a regra de webhook pegou o ponto de atenção **[verificado]** |
| Falsos positivos | 0 nos 20 mods, com as minhas regras e com as 6 do jarspect **[verificado]** |
| Regras públicas para jars e Minecraft | Quase nenhuma: 6 no jarspect, todas de campanhas específicas. As coleções grandes (Yara-Rules, Elastic, Nextron) miram PE, scripts e documentos; nenhuma tem regras de fractureiser **[verificado na busca]** |

**Veredito: não vale entrar agora.** Tudo o que o YARA-X fez no teste, a busca própria com `cafebabe` também faz, e de forma mais precisa: ela entende sequências de instruções e textos montados com `byte[]`, coisa que o YARA vê só como bytes. O YARA-X custa 23 MB e 232 crates, e não há regras públicas para importar. Vale reavaliar se surgir um repositório mantido de regras YARA para mods, ou se o dono quiser regras YARA escritas por ele mesmo (P2).

## 6. Detecção real em `.jar`: tabela consolidada

Amostras **seguras**: EICAR montado em tempo de execução e jars sintéticos com o **formato** do estágio 0 (CA-T28-01), sem função real, nunca executados (não há Java nesta máquina além do JRE portátil usado para rodar as ferramentas). Nada disso foi versionado. Nenhum malware real foi baixado, guardado ou executado.

| Amostra | Kaspersky AMSI (jar + embutidos) | Kaspersky em tempo real | `avp.com /i0` | nekodetector | jNeedle | MCAntiMalware | YARA-X (regras de teste) | Protótipo Warden (`cafebabe`) |
|---|---|---|---|---|---|---|---|---|
| `eicar.com` | ✔ | ✔ bloqueou e excluiu | (já excluído) | — | — | — | — | — |
| EICAR em jar | ✔ só por entrada | ✘ | ✔ `EICAR-Test-File` | — | — | — | — | — |
| EICAR em jar embutido | ✔ só por entrada | ✘ | ✔ | — | — | — | — | — |
| Estágio 0 (IP 2023) | ✔ | ✔ bloqueou e excluiu | (já excluído) | não leu (bloqueado) | não leu (bloqueado) | não leu | ✔ | ✔ sinal + IOC |
| Estágio 0 (TEST-NET) | ✔ | ✔ bloqueou e excluiu | (já excluído) | não leu | não leu | não leu | ✔ | ✔ sinal |
| Estágio 0 embutido (2 variantes) | ✔ (jar interno) | ✘ | ✔ `HEUR:Trojan.Java.Generic` | ✘ | ✘ (erro ao abrir o embutido) | ✘ | ✔ | ✔ sinal |
| Pontos de atenção | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✔ webhook | ✔ webhook, IP, `Runtime.exec` |
| Limpo, limpo embutido, 20 mods reais | ✘ (0 alarmes) | ✘ | ✘ (0 em 42.881) | ✘ | ✘ (0 em 20 mods) | — | ✘ | 0 sinais; 3 pontos de atenção em oficiais (Sodium, Xaero), suprimidos pela regra da ADR-0040 |
| 20 IOCs públicos por hash | — | — | — | — | — | — | — | Cymru 3, YARAify 2, CIRCL 0 (§4.2) |

✔ detectou · ✘ não detectou · — não se aplica. **[verificado]**. Ressalvas: as ferramentas não conseguiram ler os jars simples porque o Kaspersky já os tinha bloqueado (`AccessDeniedException` no jNeedle). Pelo código, o SIG1 e o SIG3 do nekodetector e do jNeedle pegariam esses jars. **[código]**

**A conclusão mais importante:** a busca própria do Warden e o antivírus local se complementam. A busca própria explica **o quê** e **onde** (classe, método, trecho), pega os pontos de atenção e não depende do computador. O antivírus traz a heurística de um fornecedor grande, atualizada todo dia, e pegou o padrão mesmo com um endereço que não estava em nenhuma lista. Nenhuma das ferramentas prontas superou as duas juntas.

## 7. Arquitetura recomendada em camadas

| Camada | Padrão | Quando roda | O que o usuário vê |
|---|---|---|---|
| **1. Conferência com o arquivo oficial** (ADR-0040, sem mudança) | Sempre ligada | Todos os arquivos | "Confere com o arquivo oficial do Modrinth / da CurseForge". **Novo:** arquivo do Modrinth cujo hash sumiu da plataforma vira "Arquivo removido da plataforma" (erro de segurança quando o `.pw.toml` diz que é dela) |
| **2. Sinais conhecidos e pontos de atenção** (W-01, ampliada) | Sempre ligada | Sinais: todos os arquivos. Pontos de atenção: só os de fora das plataformas | Como na SPEC T28. A lista passa a ter também os padrões de terceiros com licença MIT (nekodetector, jNeedle, Overwolf, 0xresetti, Mackery6969) e um tipo novo de sinal: **SHA-256 de uma classe**, além do SHA-256 do jar inteiro e de cada jar embutido |
| **3. Antivírus do computador pela AMSI** (novo) | **Ligada por padrão no Windows** quando há provedor AMSI registrado; desligável em Configurações | Jar inteiro + cada jar embutido, em todos os arquivos novos ou alterados (cache por hash, como as outras camadas). **Cada entrada**, só nos arquivos de fora das plataformas | Linha "Antivírus do computador: Kaspersky (pela AMSI)" na página. Resultado novo **"O seu antivírus considerou este arquivo perigoso"** (erro de segurança), com a parte detectada (jar, jar embutido ou classe) e a nota "O seu antivírus pode ter mostrado um aviso agora; ele veio desta checagem. O nome da ameaça aparece nos relatórios do antivírus". Sem provedor: "Nenhum antivírus respondeu pela AMSI" (informação). No Linux: "Não disponível neste sistema" |
| **3a. Reação do antivírus em disco** (novo, sem custo) | Sempre | Ao ler qualquer jar | Se o jar some ou dá "acesso negado" logo depois de baixado, o resultado é **"O antivírus bloqueou ou removeu este arquivo"** (erro de segurança), em vez de um erro genérico de leitura |
| **4. Reputação por hash na internet** (novo, opcional) | **Desligada por padrão**; liga em Configurações; com a chave do VirusTotal | Só arquivos **de fora das plataformas** e jars embutidos sem correspondência oficial; respeitando 4 consultas por minuto (fila) | "VirusTotal: 12 de 70 antivírus apontam este arquivo" (erro a partir de um limite, ex.: 3 motores) ou "não conhecido". Texto fixo: "Só a impressão digital do arquivo é enviada". Team Cymru (sem chave, SHA-1 por DNS) pode entrar no mesmo interruptor |
| YARA-X | **Fora** | — | — |
| Linha de comando do antivírus | **Fora** | — | — |

Regras gerais:
- O Warden **nunca** manda bytes de jar pela rede. Pela AMSI, os bytes vão só para o antivírus local.
- O Warden **nunca** usa EICAR nem amostra de teste no app para "testar" o antivírus.
- O resultado da AMSI entra no cache por hash, junto com a versão da lista de sinais e o nome do provedor. Trocar de antivírus invalida o cache dessa camada.
- O aviso fixo da página muda para: "O Warden confere se cada arquivo é igual ao oficial, procura sinais de programas maliciosos já conhecidos em mods e pede ao seu antivírus para examinar os arquivos. Um mod malicioso novo ainda pode passar sem ser notado. Baixe mods do Modrinth e da CurseForge sempre que puder."

## 8. Implicações para o Warden

### 8.1 Texto proposto para uma ADR nova

> **ADR-00NN — Antivírus local pela AMSI e reputação por hash na checagem de segurança dos mods**
>
> - **Status:** proposta · **Origem:** pesquisa R6 (`docs/research/07-seguranca-dos-mods.md`) · **Substitui:** a alternativa "Integrar antivírus ou VirusTotal" da ADR-0040 e a frase "nenhum arquivo é enviado a terceiros" da D27, que passa a valer como "nenhum arquivo sai do computador"; complementa a ADR-0041 (fontes da lista).
>
> **Contexto.** A ADR-0040 descartou antivírus e VirusTotal porque "envia arquivos a terceiros". A R6 mostrou que (a) a AMSI do Windows entrega bytes ao antivírus local sem rede: o Kaspersky detectou o padrão sintético do estágio 0 do fractureiser, inclusive a variante com endereço inventado e a escondida em jar embutido (entregando o jar interno separado), com 0 falsos positivos em 49 mil entradas de 20 mods reais; e (b) a consulta por hash não envia o arquivo, mas os serviços sem chave conhecem pouco de malware de Minecraft (Cymru 3 de 20 IOCs, YARAify 2 de 20), e o VirusTotal exige chave e permite 4 consultas por minuto. As ferramentas especializadas da comunidade são pequenas (3 a 17 padrões) e paradas; as de licença MIT podem ser aproveitadas como dados.
>
> **Decisão.**
> 1. Nova camada **antivírus do computador** (`warden-security::antivirus`, só Windows): `AmsiScanBuffer` sobre o jar inteiro e cada jar embutido de todo arquivo novo ou alterado, e sobre cada entrada nos arquivos de fora das plataformas. Ligada por padrão quando há provedor em `HKLM\SOFTWARE\Microsoft\AMSI\Providers`; desligável. `AMSI_RESULT_DETECTED` gera `E_SEC_ANTIVIRUS` com a parte detectada. Sem provedor: `I_SEC_NO_ANTIVIRUS`. Nunca se usa EICAR para testar a AMSI no app. Linha de comando de antivírus não é usada.
> 2. Jar que some ou fica inacessível logo depois de baixado ou lido gera `E_SEC_AV_BLOCKED` ("O antivírus bloqueou ou removeu este arquivo").
> 3. Nova camada opcional **reputação por hash** (`warden-security::reputation`, trait `ReputationLookup`): desligada por padrão; VirusTotal (chave do usuário no cofre) e Team Cymru MHR (SHA-1, sem chave) só para arquivos fora das plataformas, em fila respeitando os limites; nunca há upload. Resultado `E_SEC_REPUTATION` a partir de 3 motores ou `I_SEC_REPUTATION_UNKNOWN`.
> 4. A `signatures.toml` passa a incorporar, com atribuição em `THIRD_PARTY.md`, os padrões MIT do nekodetector, do jNeedle, do jar-infection-scanner da Overwolf, do JarMalwareScanner (0xresetti) e do Minecraft-Malware-Scanner (Mackery6969), e ganha o tipo de sinal "SHA-256 de classe". O banco do MCAntiMalware (sem licença) não é usado.
> 5. YARA-X continua fora (+23 MB, +232 crates, sem regras públicas para mods).
>
> **Consequências.** O antivírus do usuário mostra o próprio aviso quando detecta algo durante a checagem; a página explica isso. O resultado da AMSI depende do antivírus instalado e não traz o nome da ameaça. A camada de reputação é fraca para malware de Minecraft e fica opcional. Teste de integração da AMSI com EICAR em memória só na CI Windows (Defender), marcado como ignorado onde não houver provedor.

### 8.2 Mudanças nas tarefas W e na SPEC T28

**W-01** (sinais e leitura dos jars):
- Acrescentar à `signatures.toml` os padrões de terceiros (§2.3) com o campo `fonte` apontando o repositório e o commit, e a seção correspondente em `THIRD_PARTY.md` (MIT: nekodetector, jNeedle, Overwolf, 0xresetti, Mackery6969; Apache-2.0: textos do jarspect, se usados).
- Tipo de sinal novo: `hash_classe` (SHA-256 de uma `.class`), além do SHA-256 de cada jar embutido.
- Expor, para a camada 3, a lista dos jars embutidos com os bytes (para não descompactar duas vezes).
- O gerador `cargo xtask fixtures-security` pode reaproveitar `spikes/r6/probe/src/classgen.rs` e `samples.rs` (classe do estágio 0 montada sem javac). **Nunca** com EICAR em arquivo no repositório: o Kaspersky apaga e bloqueia na hora.
- Os 3 pontos de atenção `Runtime.exec` em Sodium e Xaero's Minimap entram como teste de regressão ("oficial não gera ponto de atenção").

**W-02** (conferência, página, Testar):
- Módulo `antivirus` (Windows; `cfg(windows)`; crate `windows-sys`, feature `Win32_System_Antimalware`) com trait `LocalAntivirus` para ser simulado nos testes; detecção do provedor (registro e `SecurityCenter2`); resultados `E_SEC_ANTIVIRUS`, `E_SEC_AV_BLOCKED` e `I_SEC_NO_ANTIVIRUS` no modelo de achados; cache por hash com o nome do provedor.
- Arquivo do Modrinth ausente em `POST /v2/version_files` (resposta `{}`) → "Arquivo removido da plataforma".
- Página: linha "Antivírus do computador" e o texto novo do aviso fixo (§7); nota sobre o aviso do próprio antivírus.

**Tarefa nova W-13 — Reputação por hash (opcional)**, depois da W-02: trait `ReputationLookup`, clientes VirusTotal v3 (`GET /files/{sha256}`) e Team Cymru (TXT `<sha1>.hash.cymru.com`), fila com limite de 4 por minuto, chave no cofre, interruptor em Configurações (desligado por padrão), respostas reais gravadas como fixtures (feitas pelo dono, com a chave dele) e servidor simulado.

**SPEC T28:**
- "O que o Warden faz" passa de duas para quatro partes: oficial, sinais, **antivírus do computador** e, opcional, **reputação na internet**.
- Novos resultados na tabela: "O seu antivírus considerou este arquivo perigoso" (erro), "O antivírus bloqueou ou removeu este arquivo" (erro), "Arquivo removido da plataforma" (erro, quando o `.pw.toml` diz que é da plataforma), "Apontado por N antivírus no VirusTotal" (erro, opcional).
- Aviso fixo novo (§7).
- **CA-T28-05** passa a dizer: "Nenhum byte de jar sai do computador; pela rede vão só os lotes de hashes ao Modrinth e à CurseForge (e, com a reputação ligada, hashes de arquivos de fora das plataformas ao VirusTotal e à Team Cymru)".
- CA novos: **CA-T28-10**: com um provedor AMSI simulado que aponta o jar embutido, o resultado é "O seu antivírus considerou este arquivo perigoso", com o caminho `META-INF/jars/…`; sem provedor, "Nenhum antivírus respondeu" (teste de domínio). **CA-T28-11**: na CI Windows, o EICAR em memória entregue pela AMSI real dá `DETECTED` e nenhum arquivo é gravado (integração; ignorado sem provedor). **CA-T28-12**: um jar que fica inacessível logo depois do download dá "O antivírus bloqueou ou removeu este arquivo" (teste de domínio com sistema de arquivos simulado). **CA-T28-13** (W-13): com a reputação desligada, nenhuma requisição vai ao VirusTotal nem à Team Cymru; ligada, só vão hashes de arquivos de fora das plataformas (servidor simulado que registra as requisições).
- SPEC §9: tirar "enviar arquivos a serviços de análise de terceiros (VirusTotal e afins)" da lista de adiados, porque a consulta por hash entra (W-13), e manter adiados o upload de arquivos e as regras YARA.
- Decisão D27: trocar "nenhum arquivo é enviado a terceiros (só hashes)" por "nenhum arquivo sai do computador (só hashes); o antivírus do computador examina os jars pela AMSI".

### 8.3 Decisões para o dono

1. **Pedir ao antivírus do seu computador para examinar os mods?** Quando ele achar algo, vai aparecer o aviso dele, como apareceu no teste. Com mods normais, ele não apitou nenhuma vez. **Recomendo: sim, ligado por padrão**, com opção de desligar.
2. **Consultar a "ficha" dos arquivos na internet (VirusTotal)?** Só a impressão digital é enviada, nunca o arquivo, e só para mods que não vieram do Modrinth nem da CurseForge. Exige que você crie uma conta grátis no VirusTotal e cole a chave (passo a passo na §4.4). **Recomendo: oferecer a opção, desligada por padrão**, porque os testes mostraram que esses serviços conhecem pouco de vírus de Minecraft.
3. **Quando o antivírus disser que um mod é perigoso, bloquear como um sinal conhecido?** Nesse caso o "Testar mesmo assim" sai e a publicação fica bloqueada, a menos que você use "Confiar neste arquivo". **Recomendo: sim, bloquear**, porque nos testes ele não deu alarme falso em nenhum mod real.

## 9. Fora do escopo e observações para o orquestrador

- **Backup do Kaspersky:** guarda cópias das amostras de teste (EICAR e jars sintéticos, §3.3). O dono pode apagá-las em Kaspersky → Backup e restauração. Não mexi no antivírus.
- **Armadilha do Git Bash com programas do Windows:** opções com `/` (como `/i0`) viram caminhos. Isso quase causou dano (o `avp.com` apagou conteúdo de jars de teste). Vale uma linha na QUALITY §13 ou na ADR-0048: chamar executáveis Windows com opções `/x` pelo PowerShell, ou com `MSYS2_ARG_CONV_EXCL='*'`. Não editei esses documentos.
- **Executável sem assinatura:** o Kaspersky põe programas sem assinatura no grupo "restrito" do controle de aplicativos. O Warden 1.1 deve ser assinado para não sofrer restrições em outros antivírus. **[inferência]**
- **YARAify sem chave:** respondeu sem `Auth-Key`, embora a documentação exija a chave. Não basear nada nisso.
- **jNeedle e jars embutidos gravados sem compressão:** o leitor `lljzip` falhou nos jars embutidos que o crate `zip` gravou. Nos mods reais, leu 204 jars embutidos sem erro. A causa exata não foi investigada. **[verificado o sintoma]**

## 10. O que ficou em disco

- Apagado: `C:\wt\r6\` inteiro (amostras EICAR e sintéticas, Java portátil, ferramentas, 20 mods baixados, banco SQLite do MCAntiMalware, relatórios exportados do Kaspersky, capturas de tela e as pastas `target`, uma delas de 1,3 GB). O scratchpad ficou vazio. **[verificado]**
- Ficou no git: só o código (`spikes/r6/probe`, `spikes/r6/yarax`, `spikes/r6/hashlookup`), a lista de IOCs públicos (só hashes) e este relatório. Nenhuma chave foi usada.

## Comandos executados (principais)

```text
r6probe sigscan-mem | hashes | amsi-mem | amsi-nomes | gen C:\wt\r6\amostras{,2,3} | amsi-bench <20 mods> | sigscan <20 mods>
avp.com SCAN C:\wt\r6\amostras2 /i0 /fa /RA:…        (PowerShell; saída 3, 1,4 s)
avp.com SCAN C:\wt\r6\mods /i0 /fa /RA:…             (PowerShell; 42.881 objetos, 0 detecções, 140 s)
avp.com REPORT AMSI /RA:…                            (exporta os relatórios; o nome do perfil é necessário para vir completo)
MpCmdRun.exe -Scan -ScanType 3 -File … -DisableRemediation   (0x80004005: Defender não está rodando)
java -jar jarscanner-1.1-SNAPSHOT.jar 2 <pasta>      (nekodetector)
java -jar jneedle-cli-1.1.0-local-all.jar -f <pasta> (jNeedle)
java -jar MCAntiMalware.jar --scanDirectory <pasta> --disableAutoUpdate true
r6yarax regras.yar --amostras | <20 mods>;  r6yarax jarspect-prod.yar <20 mods>
python hashlookup/consulta.py fixtures/iocs-publicos.txt
nslookup -type=TXT <md5|sha1>.hash.cymru.com
```
