import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.util.ArrayList;
import java.util.List;

/**
 * Programa de teste do spike S-R5-2: aloca memória de forma controlada.
 *
 * Aloca em blocos de 64 KB (abaixo do limite de objeto gigante do G1), para a
 * memória passar pelo eden como num jogo de verdade.
 *
 * Comandos pela entrada padrão (uma linha cada; responde "ok ..." quando termina):
 *   hold N     guarda mais N MB vivos
 *   free N     solta N MB dos guardados
 *   garbage N  aloca N MB que viram lixo na hora
 *   churn S    aloca lixo sem parar por S segundos (~200 MB/s), em segundo plano
 *   gc         chama System.gc()
 *   mem        responde a heap usada vista de dentro (Runtime), em KB
 *   quit       sai
 *
 * Compilado com --release 8 para rodar do Java 8 ao 25.
 */
public class Alloc {
    static final int CHUNK = 64 * 1024;
    static final int PER_MB = 1024 * 1024 / CHUNK;
    static final List<byte[]> held = new ArrayList<byte[]>();
    static volatile Object sink;

    public static void main(String[] args) throws Exception {
        String name = java.lang.management.ManagementFactory.getRuntimeMXBean().getName();
        System.out.println("pid " + name.substring(0, name.indexOf('@')));
        System.out.flush();
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            String[] p = line.trim().split("\\s+");
            String cmd = p[0];
            int n = p.length > 1 ? Integer.parseInt(p[1]) : 0;
            if (cmd.equals("hold")) {
                for (int i = 0; i < n * PER_MB; i++) held.add(touch(new byte[CHUNK]));
            } else if (cmd.equals("free")) {
                for (int i = 0; i < n * PER_MB && !held.isEmpty(); i++) held.remove(held.size() - 1);
            } else if (cmd.equals("garbage")) {
                for (int i = 0; i < n * PER_MB; i++) sink = touch(new byte[CHUNK]);
            } else if (cmd.equals("churn")) {
                final long end = System.currentTimeMillis() + n * 1000L;
                Thread t = new Thread(new Runnable() {
                    public void run() {
                        while (System.currentTimeMillis() < end) {
                            for (int i = 0; i < 20 * PER_MB; i++) sink = touch(new byte[CHUNK]);
                            try { Thread.sleep(100); } catch (InterruptedException e) { return; }
                        }
                    }
                });
                t.setDaemon(true);
                t.start();
            } else if (cmd.equals("gc")) {
                System.gc();
            } else if (cmd.equals("mem")) {
                Runtime rt = Runtime.getRuntime();
                System.out.println("ok mem usedKB=" + (rt.totalMemory() - rt.freeMemory()) / 1024);
                System.out.flush();
                continue;
            } else if (cmd.equals("quit")) {
                System.out.println("ok quit");
                return;
            }
            System.out.println("ok " + cmd + " held=" + held.size() / PER_MB + "MB");
            System.out.flush();
        }
    }

    static byte[] touch(byte[] b) {
        for (int i = 0; i < b.length; i += 4096) b[i] = 1;
        return b;
    }
}
