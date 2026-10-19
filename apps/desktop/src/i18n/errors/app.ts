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
  TEST_INSTANCE_CHANGED:
    'Arquivos do pack mudaram na instância de teste durante o teste anterior e seriam substituídos pela versão do pack: {{files}}. Substitua para testar, ou copie antes o que quiser guardar.',
  TEST_LOADER_UNSUPPORTED:
    'O Warden não testa packs com {{loader}}. Use Forge, NeoForge ou Fabric.',
  TEST_SESSION_NOT_FOUND:
    'Esse teste não está mais gravado. Os testes antigos que fecharam normalmente são apagados depois dos 30 mais novos.',
  TEST_INSTANCE_MISSING:
    'Este pack ainda não tem instância de teste. Clique em Testar para criá-la.',
  TEST_CAPTURE_PENDING:
    'Durante o último teste, configs do pack mudaram e ainda não foram revisadas. Revise o que mudou ou descarte antes de testar de novo.',
  CAPTURE_NOT_FOUND:
    'Não há mais mudanças deste teste para revisar: outro teste começou depois dele.',
  CAPTURE_RESOLVED: 'As mudanças deste teste já foram revisadas.',
  CAPTURE_STALE:
    'O arquivo {{path}} mudou depois que a lista apareceu. Nada foi gravado: confira a lista atualizada e tente de novo.',
  CAPTURE_INVALID:
    '{{path}} não pode ser trazido para o pack. Confira a lista atualizada e tente de novo.',
  TEST_MEMORY_HIGH_JAVA8:
    'O teste vai usar {{memoryMb}} MB com o Java 8. Acima de 8 GB, o Java 8 costuma travar o jogo por segundos nas limpezas de memória. Se acontecer, diminua a memória em Ajustes do teste.',
  TEST_ARTIFACT_INVALID: 'Esse arquivo não faz parte deste teste ou foi apagado.',
  TEST_GAME_FILES_MISSING:
    'Faltam arquivos do Minecraft para abrir o teste e não foi possível baixá-los: {{names}}. Confira a internet e tente de novo; a lista completa está nos detalhes técnicos.',
  TEST_GAME_FILES_MISSING_one:
    'Falta {{count}} arquivo do Minecraft para abrir o teste e não foi possível baixá-lo: {{names}}. Confira a internet e tente de novo; o caminho completo está nos detalhes técnicos.',
  TEST_GAME_FILES_MISSING_other:
    'Faltam {{count}} arquivos do Minecraft para abrir o teste e não foi possível baixá-los: {{names}}. Confira a internet e tente de novo; a lista completa está nos detalhes técnicos.',
  TEST_PACK_HAS_ERRORS:
    'O Warden encontrou erros no pack que podem impedir o jogo de abrir. Corrija em Problemas ou escolha testar mesmo assim.',
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
  TEST_PROFILE_NOT_FOUND:
    'Esse perfil do teste não existe mais. Abra Ajustes do teste para ver os perfis atuais.',
  TEST_PROFILE_NAME_TAKEN: 'Já existe um perfil chamado "{{name}}". Escolha outro nome.',
  TEST_PROFILE_DEFAULT_LOCKED: 'O perfil Padrão não pode ser renomeado nem apagado.',
  TEST_PROFILE_LIMIT:
    'Este pack já tem {{max}} perfis do teste. Apague um perfil que não usa para criar outro.',
  TEST_PERF_FLAGS:
    'Os argumentos {{args}} impedem o Warden de ler a memória do jogo neste teste. Para ver a memória, remova-os em Ajustes do teste.',
  TEST_MEMORY_ABOVE_RAM:
    'O teste vai usar {{memoryMb}} MB, mais de 75% da memória deste computador ({{totalMb}} MB). O computador pode ficar lento ou o jogo pode fechar. Se acontecer, diminua a memória em Ajustes do teste.',
  TEST_QUICK_PLAY_NO_WORLD:
    'O jogo vai abrir no menu principal porque ainda não há mundo de teste nesta instância. Crie um mundo no jogo: nos próximos testes, ele abre direto.',
  TEST_QUICK_PLAY_UNSUPPORTED:
    'O jogo vai abrir no menu principal: entrar direto no mundo só funciona no Minecraft 1.20 ou mais novo, e este pack usa o {{minecraft}}.',
  TEST_PLAYER_LOCAL_SOURCE:
    'O pack ainda não foi publicado para os jogadores: o teste instala o pack exportado agora, servido só neste computador. Depois de publicar, o teste usa o link do pack.',
  TEST_PLAYER_OLD_VERSION:
    'O GitHub ainda está entregando a versão anterior. Tente de novo em alguns minutos.',
  TEST_PLAYER_LINK_UNREACHABLE:
    'O link do pack não abriu. Confira a internet e se o repositório do pack no GitHub é público, e tente de novo.',
  TEST_PLAYER_BLOCKED:
    'Mods da CurseForge não baixam sozinhos para os jogadores e o Warden não tem esses arquivos: {{names}}. Baixe-os à mão (o teste normal pede) e tente de novo.',
  TEST_PLAYER_BLOCKED_one:
    '{{count}} mod da CurseForge não baixa sozinho para os jogadores e o Warden não tem esse arquivo: {{names}}. Baixe-o à mão (o teste normal pede) e tente de novo.',
  TEST_PLAYER_BLOCKED_other:
    '{{count}} mods da CurseForge não baixam sozinhos para os jogadores e o Warden não tem esses arquivos: {{names}}. Baixe-os à mão (o teste normal pede) e tente de novo.',
  AI_LINK_UNSUPPORTED:
    'O Warden só abre links do mclo.gs e do pastebin. Escolha o arquivo do log ou cole um link de um desses sites.',
  AI_LINK_NOT_HTTPS: 'Use o link do {{site}} que começa com https://.',
  AI_LINK_NEEDS_KEY:
    'O hastebin exige uma chave para ler os textos, então o Warden não aceita links dele. Peça o link do mclo.gs ou o arquivo.',
  AI_LINK_INVALID: 'Este link do {{site}} não aponta para um texto. Copie o link inteiro do log.',
  AI_LINK_NOT_FOUND: 'Este log não existe mais no {{site}}. Peça um link novo ou o arquivo.',
  AI_LINK_UNREACHABLE:
    'O {{site}} não entregou o texto do link. Confira a internet e tente de novo.',
  AI_LINK_TIMEOUT:
    'O {{site}} não respondeu em {{seconds}} segundos. Confira a internet e tente de novo.',
  AI_LINK_EMPTY: 'O link do {{site}} abriu, mas o texto está vazio.',
};
