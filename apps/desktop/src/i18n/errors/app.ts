import type { AppErrorCode } from '../../lib/ipc/bindings';
import type { ErrorPhrases } from './plural';

/** Frases dos erros do domínio `app`: o que aconteceu + o que fazer (QUALITY §3). */
export const app: ErrorPhrases<AppErrorCode> = {
  INTERNAL:
    'Algo deu errado dentro do Warden. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  OPERATION_NOT_FOUND: 'Essa tarefa não existe mais. Abra Tarefas de novo para ver a lista atual.',
  OPERATION_NOT_CANCELLABLE:
    'Essa tarefa está numa etapa que não pode ser interrompida. Espere ela terminar.',
  SETTINGS_INVALID: 'Um valor das configurações foi recusado. Confira o campo e tente de novo.',
  SETTINGS_NEWER_VERSION:
    'As configurações foram salvas por uma versão mais nova do Warden e não podem ser alteradas nesta. Atualize o Warden.',
  GAME_ALREADY_RUNNING:
    'Já existe um jogo em execução (pack {{packName}}). Só dá para testar um pack por vez: feche o jogo ou vá para o teste dele.',
  NO_GAME_RUNNING: 'Não há jogo aberto neste pack. Ele pode ter fechado agora há pouco.',
  TEST_MODE_UNAVAILABLE:
    'Este tipo de teste ainda não está disponível nesta versão do Warden. Use o Testar normal.',
  TEST_INSTANCE_CHANGED:
    'Arquivos do pack mudaram na instância de teste durante o teste anterior e seriam substituídos pela versão do pack: {{files}}. Substitua para testar, ou copie antes o que quiser guardar.',
  TEST_LOADER_UNSUPPORTED:
    'O Warden não testa packs com {{loader}}. Use Forge, NeoForge ou Fabric.',
  TEST_SESSION_NOT_FOUND:
    'Esse teste não está mais gravado. Os testes antigos que fecharam normalmente são apagados depois dos 30 mais novos.',
  TEST_INSTANCE_MISSING:
    'Este pack ainda não tem instância de teste. Clique em Testar para criá-la.',
  TEST_MEMORY_HIGH_JAVA8:
    'O teste vai usar {{memoryMb}} MB com o Java 8. Acima de 8 GB, o Java 8 costuma travar o jogo por segundos nas limpezas de memória. Se acontecer, diminua a memória em Ajustes do teste.',
  TEST_ARTIFACT_INVALID: 'Esse arquivo não faz parte deste teste ou foi apagado.',
  TEST_GAME_FILES_MISSING:
    'Faltam arquivos do Minecraft para abrir o teste e não foi possível baixá-los: {{names}}. Confira a internet e tente de novo; a lista completa está nos detalhes técnicos.',
  TEST_GAME_FILES_MISSING_one:
    'Falta {{count}} arquivo do Minecraft para abrir o teste e não foi possível baixá-lo: {{names}}. Confira a internet e tente de novo; o caminho completo está nos detalhes técnicos.',
  TEST_GAME_FILES_MISSING_other:
    'Faltam {{count}} arquivos do Minecraft para abrir o teste e não foi possível baixá-los: {{names}}. Confira a internet e tente de novo; a lista completa está nos detalhes técnicos.',
  MANUAL_DOWNLOAD_NOT_WAITING:
    'Este mod não está mais esperando o download manual: o teste foi cancelado ou já continuou.',
  MANUAL_DOWNLOAD_NO_PAGE:
    'O Warden não sabe a página de download deste mod. Procure o mod pelo nome no site da CurseForge e baixe o arquivo indicado.',
  STORAGE_GAME_RUNNING:
    'O cache não foi limpo porque o jogo do pack {{packName}} está aberto. Feche o jogo e tente de novo.',
  STORAGE_OPERATION_RUNNING:
    'O cache não foi limpo porque há tarefas em andamento. Espere elas terminarem (veja em Tarefas) e tente de novo.',
  STORAGE_OPERATION_RUNNING_one:
    'O cache não foi limpo porque há {{count}} tarefa em andamento. Espere ela terminar (veja em Tarefas) e tente de novo.',
  STORAGE_OPERATION_RUNNING_other:
    'O cache não foi limpo porque há {{count}} tarefas em andamento. Espere elas terminarem (veja em Tarefas) e tente de novo.',
};
