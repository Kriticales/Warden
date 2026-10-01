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
| [0008](0008-fontes-de-mods.md) | Fontes de mods | dono |
| [0009](0009-exportacao-nativa-e-separacao.md) | Exportação nativa e separação entre projeto e instância | dono |
| [0010](0010-launcher-offline-motor-abstrato.md) | Launcher integrado, só offline, com motor abstrato | dono + técnica |
| [0011](0011-materializacao-propria.md) | Materialização do pack em Rust com conformidade ao packwiz-installer | técnica |
| [0012](0012-gestao-de-java.md) | Gestão de Java | técnica |
| [0013](0013-editor-de-configs.md) | Editor de configs com preservação de formato | dono + técnica |
| [0014](0014-diagnostico-e-ia.md) | Diagnóstico determinístico e IA com consentimento | dono |
| [0015](0015-versionamento-sobre-git.md) | Versionamento simples sobre git embutido | dono + técnica |
| [0016](0016-interface-pt-br.md) | Interface só em português do Brasil | dono + técnica |
| [0017](0017-segredos-no-cofre.md) | Segredos no cofre de credenciais do sistema | dono |
| [0018](0018-contrato-tipado-e-erros.md) | Contrato tipado e modelo de erros | técnica |
| [0019](0019-concorrencia-e-cancelamento.md) | Concorrência, travas por pack e cancelamento | técnica |
| [0020](0020-bibliotecas-frontend.md) | Bibliotecas do frontend | técnica |
| [0021](0021-layout-em-disco.md) | Layout em disco e metadados do Warden | técnica |
| [0022](0022-testes-e-ci.md) | Estratégia de testes e CI | técnica |
| [0023](0023-registros-e-privacidade.md) | Registros e privacidade | técnica |
| [0024](0024-bibliotecas-rust.md) | Bibliotecas Rust principais | técnica |

Decisões ainda abertas para o dono estão em [SPEC §10](../SPEC.md#10-decisões-pendentes-do-dono). A escolha do motor do launcher sai do spike S1 e será registrada num ADR novo pela tarefa L-02.
