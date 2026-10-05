import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.lang.management.ManagementFactory;
import java.nio.charset.Charset;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * Jogo simulado dos testes da warden-launcher (ROADMAP L-02, CA-T13-02, CA-T13-03, CA-T13-07).
 *
 * Compilado para Java 8 (roda do 8 ao 25):
 *   javac --release 8 -encoding UTF-8 -d . WardenFakeGame.java
 *
 * Modos (primeiro argumento):
 *   acentos            escreve linhas com acentos no stdout e no stderr (texto e evento XML do
 *                      log4j) e sai com 0;
 *   filhos N           abre N processos filhos (este programa no modo "dormir"), que herdam o
 *                      stdout, e fica vivo escrevendo um batimento;
 *   dormir             filho: escreve o pid e um batimento a cada 200 ms, para sempre;
 *   travar             escreve "pronto" e fica parado num laço sem fim (jogo travado);
 *   args               escreve cada argumento recebido, um por linha, entre colchetes;
 *   sair N             sai com o código N;
 *   crash              grava crash-reports/crash-teste-client.txt e sai com -1;
 *   uuid NOME...       escreve o UUID offline de cada nome, como o Minecraft calcula.
 */
public final class WardenFakeGame {
    private WardenFakeGame() {}

    private static String pid() {
        String name = ManagementFactory.getRuntimeMXBean().getName();
        int at = name.indexOf('@');
        return at > 0 ? name.substring(0, at) : name;
    }

    public static void main(String[] args) throws Exception {
        String mode = args.length > 0 ? args[0] : "";
        if (mode.equals("acentos")) {
            System.out.println("[12:00:00] [main/INFO] [teste/]: Ação, coração, pé, avô, vovó, pão, Über, ñ");
            System.out.println("<log4j:Event logger=\"teste\" timestamp=\"1\" level=\"WARN\" thread=\"Render thread\">");
            System.out.println("  <log4j:Message><![CDATA[Configuração inválida: ç ã õ é]]></log4j:Message>");
            System.out.println("</log4j:Event>");
            System.err.println("Exceção: não foi possível ler o arquivo 'café.txt'");
            System.out.println("file.encoding=" + System.getProperty("file.encoding")
                    + " stdout.encoding=" + System.getProperty("stdout.encoding")
                    + " padrão=" + Charset.defaultCharset());
            System.out.flush();
            System.err.flush();
            System.exit(0);
        } else if (mode.equals("filhos")) {
            int count = Integer.parseInt(args[1]);
            String java = System.getProperty("java.home") + File.separator + "bin" + File.separator + "java";
            String classpath = System.getProperty("java.class.path");
            List<Process> children = new ArrayList<Process>();
            for (int i = 0; i < count; i++) {
                ProcessBuilder builder = new ProcessBuilder(java, "-cp", classpath, "WardenFakeGame", "dormir");
                builder.inheritIO();
                children.add(builder.start());
            }
            System.out.println("pai " + pid() + " abriu " + children.size() + " filhos");
            System.out.flush();
            while (true) {
                Thread.sleep(200);
                System.out.println("pai vivo");
                System.out.flush();
            }
        } else if (mode.equals("dormir")) {
            System.out.println("filho " + pid());
            System.out.flush();
            while (true) {
                Thread.sleep(200);
                System.out.println("filho vivo");
                System.out.flush();
            }
        } else if (mode.equals("travar")) {
            System.out.println("pronto " + pid());
            System.out.flush();
            long counter = 0;
            while (true) {
                counter++;
                if (counter == Long.MIN_VALUE) {
                    System.out.println(counter);
                }
            }
        } else if (mode.equals("args")) {
            for (int i = 1; i < args.length; i++) {
                System.out.println("[" + args[i] + "]");
            }
            System.out.println("propriedade=" + System.getProperty("warden.teste"));
            System.exit(0);
        } else if (mode.equals("sair")) {
            System.exit(Integer.parseInt(args[1]));
        } else if (mode.equals("crash")) {
            File dir = new File("crash-reports");
            dir.mkdirs();
            Writer writer = new OutputStreamWriter(new FileOutputStream(new File(dir, "crash-teste-client.txt")), "UTF-8");
            writer.write("---- Minecraft Crash Report ----\nDescription: Initializing game\n");
            writer.close();
            System.out.println("#@!@# Game crashed! Crash report saved to: crash-reports" + File.separator + "crash-teste-client.txt");
            System.exit(-1);
        } else if (mode.equals("uuid")) {
            for (int i = 1; i < args.length; i++) {
                UUID uuid = UUID.nameUUIDFromBytes(("OfflinePlayer:" + args[i]).getBytes("UTF-8"));
                System.out.println(args[i] + "=" + uuid);
            }
            System.exit(0);
        } else {
            System.err.println("modo desconhecido: " + mode);
            System.exit(2);
        }
    }
}
