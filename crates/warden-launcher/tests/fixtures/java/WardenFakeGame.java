import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.lang.management.ManagementFactory;
import java.net.Socket;
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
 *   marcadores MS      espera MS ms, escreve os marcadores de "carregou" do Minecraft 1.13+
 *                      (som e atlas de blocos), espera mais MS/2 ms, escreve o de "entrou no
 *                      mundo", ocupa 64 MB de memória e fica vivo escrevendo um batimento
 *                      (L-10, tempos de carregamento e RAM do processo);
 *   args               escreve cada argumento recebido, um por linha, entre colchetes;
 *   sair N             sai com o código N;
 *   crash              grava crash-reports/crash-teste-client.txt e sai com -1;
 *   crash-npe          grava um crash report "Ticking entity" com NullPointerException num mod,
 *                      com números (linhas da pilha, índice do jar, coordenadas, horário e
 *                      nome do arquivo) diferentes a cada execução, e sai com -1 (D-06: a
 *                      mesma causa com números diferentes vira uma linha só em Travamentos);
 *   uuid NOME...       escreve o UUID offline de cada nome, como o Minecraft calcula;
 *   conectar           entra no servidor do Quick Play (`--quickPlayMultiplayer host:porta`,
 *                      ou `--server`/`--port` antes da 1.20): conecta e manda o nome do
 *                      jogador (`--username`), como o servidor simulado da warden-server espera,
 *                      e fica vivo escrevendo um batimento (L-09, Testar como servidor).
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
        } else if (mode.equals("marcadores")) {
            long delay = Long.parseLong(args[1]);
            System.out.println("[main/INFO]: Jogo simulado " + pid() + " carregando");
            System.out.flush();
            Thread.sleep(delay);
            System.out.println("[Render thread/INFO]: Sound engine started");
            System.out.println("[Render thread/INFO]: Created: 1024x1024x4 minecraft:textures/atlas/blocks.png-atlas");
            System.out.flush();
            Thread.sleep(delay / 2);
            System.out.println("[Server thread/INFO]: Jogador[local:E:1] logged in with entity id 1 at (0.5, 64.0, 0.5)");
            System.out.flush();
            byte[] memory = new byte[64 * 1024 * 1024];
            for (int i = 0; i < memory.length; i += 4096) {
                memory[i] = 1;
            }
            while (true) {
                Thread.sleep(200);
                System.out.println("jogo vivo " + memory.length);
                System.out.flush();
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
        } else if (mode.equals("crash-npe")) {
            long n = (System.nanoTime() / 1000L) % 9000L + Long.parseLong(pid()) % 1000L;
            File dir = new File("crash-reports");
            dir.mkdirs();
            String name = "crash-npe-" + pid() + "-client.txt";
            Writer writer = new OutputStreamWriter(new FileOutputStream(new File(dir, name)), "UTF-8");
            writer.write("---- Minecraft Crash Report ----\n"
                    + "// Who set us up the TNT?\n\n"
                    + "Time: 2026-10-13 12:" + (n % 60) + ":" + (n % 59) + "\n"
                    + "Description: Ticking entity\n\n"
                    + "java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local" + (n % 7) + ">\" is null\n"
                    + "\tat com.example.meumod.EntityTicker.tick(EntityTicker.java:" + (40 + n) + ") ~[meumod-1.0.jar%23" + (100 + n) + "!/:?]\n"
                    + "\tat net.minecraft.world.level.Level.guardEntityTick(Level.java:" + (500 + n) + ") ~[client-1.20.1-srg.jar%23185!/:?]\n\n"
                    + "-- Entity being ticked --\n"
                    + "Details:\n"
                    + "\tEntity Type: meumod:golem (com.example.meumod.Golem)\n"
                    + "\tEntity's Exact location: " + (n * 3) + ".50, 64.00, -" + n + ".25\n");
            writer.close();
            System.out.println("#@!@# Game crashed! Crash report saved to: crash-reports" + File.separator + name);
            System.exit(-1);
        } else if (mode.equals("conectar")) {
            String host = null;
            int port = 25565;
            String name = "Jogador";
            for (int i = 1; i + 1 < args.length; i++) {
                if (args[i].equals("--quickPlayMultiplayer")) {
                    String target = args[i + 1];
                    int colon = target.lastIndexOf(':');
                    host = colon > 0 ? target.substring(0, colon) : target;
                    port = colon > 0 ? Integer.parseInt(target.substring(colon + 1)) : 25565;
                } else if (args[i].equals("--server")) {
                    host = args[i + 1];
                } else if (args[i].equals("--port")) {
                    port = Integer.parseInt(args[i + 1]);
                } else if (args[i].equals("--username")) {
                    name = args[i + 1];
                }
            }
            if (host == null) {
                System.out.println("[Render thread/INFO]: sem servidor para entrar");
                System.exit(3);
            }
            System.out.println("[Render thread/INFO]: Connecting to " + host + ", " + port);
            System.out.flush();
            Socket socket = new Socket(host, port);
            Writer writer = new OutputStreamWriter(socket.getOutputStream(), "UTF-8");
            writer.write(name + "\n");
            writer.flush();
            while (true) {
                Thread.sleep(200);
                System.out.println("jogo conectado");
                System.out.flush();
            }
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
