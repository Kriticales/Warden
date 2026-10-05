import type { JavaErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `java` (QUALITY §3): escolher, baixar, conferir e remover o Java
 * do teste. Dona: L-01. Parâmetros: `source` (Adoptium ou Mojang, sempre entre parênteses, sem artigo), `major`, `maxUpdate`,
 * `platform`, `path`, `id`, `expected`, `found`, `minecraft`.
 */
export const java: Record<JavaErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao preparar o Java. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  UNSUPPORTED_PLATFORM:
    'O Warden não tem um Java de 64 bits para este computador ({{platform}}). O teste do jogo só funciona no Windows ou no Linux de 64 bits.',
  NO_BUILD_AVAILABLE:
    'A fonte de downloads ({{source}}) não publica o Java {{major}} para este computador. Escolha outro Java em Ajustes do teste ou tente de novo mais tarde.',
  SOURCE_UNAVAILABLE:
    'A fonte de downloads do Java ({{source}}) está fora do ar ou recusou o pedido. Tente de novo em alguns minutos.',
  INVALID_RESPONSE:
    'A fonte de downloads do Java ({{source}}) respondeu algo que o Warden não entende. Tente de novo mais tarde; se continuar, copie os detalhes técnicos e registre o problema.',
  DOWNLOAD_CORRUPTED:
    'O Java baixado ({{source}}) veio diferente do publicado e foi descartado. Tente de novo; se continuar, a sua conexão pode estar alterando os downloads.',
  ARCHIVE_INVALID:
    'O pacote do Java baixado está estragado ou não tem o programa do Java, e nada foi instalado. Tente de novo.',
  VALIDATION_FAILED:
    'O Java não abriu quando o Warden foi conferi-lo. Tente de novo; se continuar, veja os detalhes técnicos (um antivírus pode estar bloqueando o arquivo).',
  NOT_64_BIT:
    'Este Java é de 32 bits, e o Warden só usa Java de 64 bits. Escolha outro Java em Ajustes do teste.',
  UNEXPECTED_VERSION:
    'O Java baixado é a versão {{found}}, mas o teste precisa do Java {{expected}}. Nada foi instalado; tente de novo.',
  RUNTIME_NOT_FOUND:
    'Este Java não está mais instalado. Atualize a lista de Javas em Configurações.',
  RUNTIME_IN_USE: 'Um jogo aberto está usando este Java. Feche o jogo e tente remover de novo.',
  INSTALL_BLOCKED:
    'Outro programa está segurando os arquivos do Java e o Warden não conseguiu terminar a instalação. Espere um pouco e tente de novo; se continuar, confira se o antivírus está bloqueando a pasta do Warden.',
  VERSION_REQUIREMENT_UNKNOWN:
    'O Warden não sabe qual Java usar com o Minecraft {{minecraft}}. Escolha um Java em Ajustes do teste.',
  COMPATIBILITY_TABLE_INVALID:
    'A tabela de Javas do Warden está com defeito. Registre o problema com os detalhes técnicos.',
};
