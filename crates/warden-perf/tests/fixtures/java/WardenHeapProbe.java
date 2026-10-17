import java.util.ArrayList;
import java.util.List;

/**
 * Programa de teste da memória da JVM (ROADMAP L-11; CA-T13-10 e CA-T13-11).
 *
 * Compilado para Java 8 (roda do 8 ao 25):
 *   javac --release 8 -encoding UTF-8 -d . WardenHeapProbe.java
 *
 * Modos (primeiro argumento):
 *   parar MB       guarda MB megabytes vivos (blocos de 64 KB), chama System.gc(), escreve
 *                  "pronto" e fica parado, sem alocar nada (momento estável para comparar com
 *                  o jstat);
 *   cheia PCT      guarda PCT% da heap máxima vivos, escreve "pronto" e fica alocando lixo pequeno
 *                  sem parar: cada coleta deixa a heap em PCT% (memória quase cheia).
 */
public final class WardenHeapProbe {
    private static final int BLOCK = 64 * 1024;
    private static volatile Object sink;
    /** Os blocos vivos: num campo estático, para o JIT não os dar por mortos. */
    private static volatile List<byte[]> live;

    private WardenHeapProbe() {}

    private static List<byte[]> hold(long bytes) {
        List<byte[]> live = new ArrayList<byte[]>();
        for (long held = 0; held < bytes; held += BLOCK) {
            byte[] block = new byte[BLOCK];
            block[0] = 1;
            live.add(block);
        }
        return live;
    }

    public static void main(String[] args) throws Exception {
        String mode = args.length > 0 ? args[0] : "";
        if (mode.equals("parar")) {
            live = hold(Long.parseLong(args[1]) * 1024 * 1024);
            System.gc();
            System.out.println("pronto " + live.size());
            System.out.flush();
            while (true) {
                Thread.sleep(1000);
            }
        } else if (mode.equals("cheia")) {
            long max = Runtime.getRuntime().maxMemory();
            live = hold(max / 100 * Long.parseLong(args[1]));
            System.out.println("pronto " + live.size());
            System.out.flush();
            while (true) {
                for (int i = 0; i < 16; i++) {
                    sink = new byte[16 * 1024];
                }
                Thread.sleep(2);
            }
        } else {
            System.err.println("modo desconhecido: " + mode);
            System.exit(2);
        }
    }
}
