# Registro de decisões (ADRs)

Formato e regras em [ADR-0001](0001-registro-de-decisoes.md). "Decisão do dono" só muda com nova decisão do dono.

| ADR | Título | Origem |
|---|---|---|
| [0001](0001-registro-de-decisoes.md) | Registro de decisões de arquitetura | técnica |
| [0002](0002-stack-tauri-rust-react.md) | Stack: Tauri 2, Rust e React com TypeScript | dono |
| [0003](0003-plataformas-windows-linux-wsl.md) | Plataformas: Windows primeiro, Linux depois, desenvolvimento em WSL2 | dono + técnica |
| [0004](0004-uso-privado-e-licencas.md) | Uso privado e política de licenças de terceiros | dono |
| [0005](0005-versoes-e-loaders.md) | Versões do Minecraft e loaders suportados | dono |
| [0006](0006-integracao-hibrida-packwiz.md) | Integração híbrida com o packwiz | dono + técnica |
| [0007](0007-sidecar-packwiz.md) | packwiz como sidecar de commit fixado, chave da CurseForge em tempo de execução | dono + técnica |
| [0008](0008-fontes-de-mods.md) | Fontes de mods (busca: ver 0027) | dono |
| [0009](0009-exportacao-nativa-e-separacao.md) | Exportação nativa e separação entre projeto e instância | dono |
| [0010](0010-launcher-offline-motor-abstrato.md) | Launcher integrado, só offline, com motor abstrato | dono + técnica |
| [0011](0011-materializacao-propria.md) | Materialização do pack em Rust com conformidade ao packwiz-installer | técnica |
| [0012](0012-gestao-de-java.md) | Gestão de Java (escolha da versão: ver 0029) | técnica |
| [0013](0013-editor-de-configs.md) | Editor de configs com preservação de formato | dono + técnica |
| [0014](0014-diagnostico-e-ia.md) | Diagnóstico determinístico e IA com consentimento (IA: ver 0030) | dono |
| [0015](0015-versionamento-sobre-git.md) | Versionamento simples sobre git embutido (GitHub: ver 0028) | dono + técnica |
| [0016](0016-interface-pt-br.md) | Interface só em português do Brasil | dono + técnica |
| [0017](0017-segredos-no-cofre.md) | Segredos no cofre de credenciais do sistema (substituída pela 0025) | dono |
| [0018](0018-contrato-tipado-e-erros.md) | Contrato tipado e modelo de erros | técnica |
| [0019](0019-concorrencia-e-cancelamento.md) | Concorrência, travas por pack e cancelamento | técnica |
| [0020](0020-bibliotecas-frontend.md) | Bibliotecas do frontend | técnica |
| [0021](0021-layout-em-disco.md) | Layout em disco e metadados do Warden | técnica |
| [0022](0022-testes-e-ci.md) | Estratégia de testes e CI | técnica |
| [0023](0023-registros-e-privacidade.md) | Registros e privacidade | técnica |
| [0024](0024-bibliotecas-rust.md) | Bibliotecas Rust principais | técnica |
| [0025](0025-segredos-cofre-ou-env.md) | Segredos: cofre do sistema por padrão, arquivo .env opcional | dono |
| [0026](0026-estrutura-de-navegacao.md) | Estrutura de navegação do app (funções avançadas: ver 0036) | dono |
| [0027](0027-busca-combinada.md) | Busca de mods combinada (Modrinth + CurseForge) | dono |
| [0028](0028-publicacao-no-github.md) | Publicar versões no GitHub para distribuir o pack | dono |
| [0029](0029-java-mais-novo-que-funciona.md) | Java: o mais novo que funciona, com o motivo à vista | dono |
| [0030](0030-ia-com-ferramentas-e-conversas.md) | IA "médico" com ferramentas, conversas por pack e consentimento por conversa | dono |
| [0031](0031-busca-do-culpado.md) | Busca do culpado por rodadas (bisseção respeitando dependências) | dono + técnica |
| [0032](0032-servidor-local-sob-demanda.md) | Servidor local sob demanda e EULA do Minecraft | dono |
| [0033](0033-mods-iniciais-e-kits.md) | Mods iniciais (spark e Crash Assistant), ferramentas do jogador e kits de desempenho | dono |
| [0034](0034-analise-estatica-do-pack.md) | Análise estática do pack: raio-x de mixins, grafo e nota de saúde | técnica + dono |
| [0035](0035-importar-e-pacote-para-servidor.md) | Importar modpacks de outros apps e gerar o pacote para servidor | dono |
| [0036](0036-funcoes-avancadas-na-estrutura.md) | Funções avançadas encaixadas na estrutura aprovada | dono |
| [0037](0037-editor-de-scripts.md) | Editor de scripts KubeJS e CraftTweaker | dono + técnica |
| [0038](0038-monitor-de-desempenho.md) | Monitor de desempenho do jogo sem JDK | técnica |
| [0039](0039-warden-1-1-profissional.md) | Warden 1.1 "Profissional": versão nova e ganchos na v1 | dono + técnica |
| [0040](0040-seguranca-dos-mods.md) | Checagem de segurança dos mods | dono + técnica |
| [0041](0041-lista-de-sinais-embutida.md) | Lista de sinais embutida e atualizada com o app | técnica |
| [0042](0042-manutencao-e-substitutos.md) | Manutenção dos mods e substitutos | dono |
| [0043](0043-travamento-de-jogador.md) | Analisar o travamento de um jogador | dono |
| [0044](0044-notas-e-grupos.md) | Notas e grupos em `.warden/mods.toml` | técnica |
| [0045](0045-funcoes-1-1-na-estrutura.md) | Funções da 1.1 encaixadas na estrutura aprovada | dono |
| [0046](0046-itens-repetidos.md) | Itens repetidos: detecção estática e peso na saúde | dono + técnica |
| [0047](0047-desempenho-entre-versoes.md) | Desempenho entre versões | técnica |

As decisões do dono estão em [SPEC §10](../SPEC.md#10-decisões-do-dono). A escolha do motor do launcher sai do spike S1 e será registrada num ADR novo pela tarefa L-02.
