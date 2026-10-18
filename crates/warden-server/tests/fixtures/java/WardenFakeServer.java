import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.lang.management.ManagementFactory;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.Properties;

/**
 * Servidor simulado dos testes da warden-server e do E2E (ROADMAP L-09: "processo Java de teste
 * que imprime Done"). Imita o que o Warden lê de um servidor do Minecraft de verdade:
 *
 * - sem eula=true no eula.txt, escreve o aviso da Mojang e sai com 0 (como o servidor real);
 * - lê server-ip e server-port do server.properties e escuta nesse endereço (a porta fica
 *   ocupada enquanto ele estiver aberto);
 * - escreve o mundo que abre ("Preparing level", com "(mundo existente)" quando a pasta do
 *   level-name já tem o level.dat);
 * - escreve "Done (…)! For help, type "help"" e passa a ler comandos do stdin: "stop" fecha
 *   (salvando o "mundo"), qualquer outro é ecoado;
 * - cada conexão recebida lê o nome do jogador (uma linha) e escreve as linhas de entrada no
 *   servidor ("logged in with entity id", "joined the game").
 *
 * Compilado para Java 8 (roda do 8 ao 25):
 *   javac --release 8 -encoding UTF-8 -d . WardenFakeServer.java
 *
 * Modos (primeiro argumento; sem argumento, "normal"):
 *   normal             abre e fica esperando comandos;
 *   lento MS           espera MS ms antes do Done;
 *   ignorar-stop       responde ao "stop" sem fechar (o Warden encerra à força);
 *   travar-cliente     ao abrir, um mod de cliente derruba o servidor: grava o crash report
 *                      (crash-reports/crash-teste-server.txt) e sai com 1;
 *   sair N             sai com o código N antes do Done;
 *   culpado NOME...    quando há em mods/ um jar começando por cada NOME, o jogador não
 *                      consegue entrar: escreve "Couldn't place player in world" e a exceção
 *                      (como o servidor de verdade) e continua aberto; senão o jogador entra
 *                      normalmente (D-13, busca do culpado com o servidor local).
 */
public final class WardenFakeServer {
    private WardenFakeServer() {}

    private static String pid() {
        String name = ManagementFactory.getRuntimeMXBean().getName();
        int at = name.indexOf('@');
        return at > 0 ? name.substring(0, at) : name;
    }

    private static void log(String level, String text) {
        System.out.println("[12:00:00] [Server thread/" + level + "]: " + text);
        System.out.flush();
    }

    private static boolean eulaAccepted() throws Exception {
        File file = new File("eula.txt");
        if (!file.isFile()) {
            return false;
        }
        Properties eula = new Properties();
        FileInputStream in = new FileInputStream(file);
        try {
            eula.load(in);
        } finally {
            in.close();
        }
        return "true".equals(eula.getProperty("eula", "").trim());
    }

    private static Properties properties() throws Exception {
        Properties properties = new Properties();
        File file = new File("server.properties");
        if (file.isFile()) {
            FileInputStream in = new FileInputStream(file);
            try {
                properties.load(in);
            } finally {
                in.close();
            }
        }
        return properties;
    }

    private static void crashWithClientMod() throws Exception {
        File dir = new File("crash-reports");
        dir.mkdirs();
        String report = "---- Minecraft Crash Report ----\n"
                + "// Who set us up the TNT?\n\n"
                + "Time: 2026-10-13 12:00:00\n"
                + "Description: Exception in server tick loop\n\n"
                + "java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER\n"
                + "\tat net.minecraftforge.fml.loading.RuntimeDistCleaner.processClassWithFlags(RuntimeDistCleaner.java:57) ~[fmlloader-1.20.1-47.3.0.jar%23103!/:1.0] {}\n"
                + "\tat com.example.zoomcliente.ZoomMod.<init>(ZoomMod.java:21) ~[zoomcliente-1.0.jar%23130!/:1.0] {re:classloading}\n";
        Writer writer = new OutputStreamWriter(
                new FileOutputStream(new File(dir, "crash-teste-server.txt")), "UTF-8");
        writer.write(report);
        writer.close();
        log("ERROR", "Encountered an unexpected exception");
        System.out.println("java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER");
        System.out.println("\tat com.example.zoomcliente.ZoomMod.<init>(ZoomMod.java:21) ~[zoomcliente-1.0.jar%23130!/:1.0] {re:classloading}");
        log("ERROR", "This crash report has been saved to: " + new File(dir, "crash-teste-server.txt").getPath());
        System.out.flush();
        System.exit(1);
    }

    /** Se há em mods/ um jar começando por cada nome de `names` (e pelo menos um nome). */
    private static boolean culpritsPresent(String[] names) {
        String[] jars = new File("mods").list();
        if (names.length == 0 || jars == null) {
            return false;
        }
        for (String name : names) {
            boolean found = false;
            for (String jar : jars) {
                if (jar.startsWith(name)) {
                    found = true;
                }
            }
            if (!found) {
                return false;
            }
        }
        return true;
    }

    public static void main(String[] args) throws Exception {
        String mode = args.length > 0 ? args[0] : "normal";
        final String[] culprits = new String[mode.equals("culpado") ? args.length - 1 : 0];
        if (culprits.length > 0) {
            System.arraycopy(args, 1, culprits, 0, culprits.length);
        }
        log("INFO", "Starting minecraft server version 1.20.1 (simulado, pid " + pid() + ")");
        if (!eulaAccepted()) {
            log("INFO", "You need to agree to the EULA in order to run the server. Go to eula.txt for more info.");
            System.exit(0);
        }
        if (mode.equals("sair")) {
            System.exit(Integer.parseInt(args[1]));
        }
        if (mode.equals("travar-cliente")) {
            crashWithClientMod();
        }
        Properties properties = properties();
        String ip = properties.getProperty("server-ip", "").trim();
        int port = Integer.parseInt(properties.getProperty("server-port", "25565").trim());
        log("INFO", "Loading properties");
        log("WARN", "**** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!");
        log("INFO", "online-mode=" + properties.getProperty("online-mode")
                + " enable-rcon=" + properties.getProperty("enable-rcon")
                + " enable-query=" + properties.getProperty("enable-query")
                + " motd=" + properties.getProperty("motd"));
        final ServerSocket socket = new ServerSocket(port, 50, InetAddress.getByName(ip.isEmpty() ? "0.0.0.0" : ip));
        log("INFO", "Starting Minecraft server on " + (ip.isEmpty() ? "*" : ip) + ":" + port);
        if (mode.equals("lento")) {
            Thread.sleep(Long.parseLong(args[1]));
        }
        String level = properties.getProperty("level-name", "world").trim();
        log("INFO", "Preparing level \"" + level + "\""
                + (new File(level, "level.dat").isFile() ? " (mundo existente)" : ""));
        log("INFO", "Done (0.512s)! For help, type \"help\"");

        Thread accept = new Thread(new Runnable() {
            public void run() {
                int entity = 1;
                while (!socket.isClosed()) {
                    try {
                        Socket client = socket.accept();
                        BufferedReader in = new BufferedReader(new InputStreamReader(client.getInputStream(), "UTF-8"));
                        String name = in.readLine();
                        if (name == null || name.trim().isEmpty()) {
                            name = "Jogador";
                        }
                        if (culpritsPresent(culprits)) {
                            log("ERROR", "Couldn't place player in world");
                            System.out.println("java.lang.IllegalStateException: conflito simulado de " + culprits.length + " mods no servidor");
                            System.out.println("\tat com.example.culpado.Servidor.entrar(Servidor.java:42)");
                            System.out.flush();
                            continue;
                        }
                        log("INFO", name.trim() + "[/127.0.0.1:" + client.getPort() + "] logged in with entity id " + entity + " at (0.5, 64.0, 0.5)");
                        log("INFO", name.trim() + " joined the game");
                        entity++;
                    } catch (Exception closed) {
                        return;
                    }
                }
            }
        });
        accept.setDaemon(true);
        accept.start();

        BufferedReader stdin = new BufferedReader(new InputStreamReader(System.in, "UTF-8"));
        String line;
        while ((line = stdin.readLine()) != null) {
            String command = line.trim();
            if (command.equals("stop")) {
                log("INFO", "Stopping the server");
                if (mode.equals("ignorar-stop")) {
                    log("INFO", "Saving worlds (simulado: não fecha)");
                    continue;
                }
                log("INFO", "Saving worlds");
                socket.close();
                System.exit(0);
            }
            log("INFO", "Comando recebido: " + command);
        }
        // Sem stdin (fechado): fica aberto até ser encerrado, como o servidor real.
        while (true) {
            Thread.sleep(200);
        }
    }
}
