package dev.kriticales.wardenfalhas;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/**
 * Mod de teste do spike S-R5-3 (Warden). Provoca falhas reais e controladas para gravar
 * os sinais que a busca do culpado precisa reconhecer. O modo vem de -Dwarden.falha=...
 * ou do arquivo /wardenfalhas.txt dentro do jar:
 *   mixin      injeção que não acha o método alvo (falha do Mixin na inicialização)
 *   iniciar    exceção no construtor do cliente (trava ao abrir)
 *   entrar     exceção ao criar o jogador no servidor (trava ao entrar no mundo)
 *   travar     laço infinito ao criar o jogador (o jogo para de responder, sem fechar)
 *   par:CLASSE como "entrar", mas só se a CLASSE de outro mod existir (conflito de 2 mods)
 */
public final class Falhas {
    private Falhas() {}

    public static String modo() {
        String p = System.getProperty("warden.falha");
        if (p != null) return p.trim();
        try (InputStream in = Falhas.class.getResourceAsStream("/wardenfalhas.txt")) {
            if (in != null) return new String(in.readAllBytes(), StandardCharsets.UTF_8).trim();
        } catch (Exception e) {
            // sem arquivo: nenhum modo
        }
        return "nenhuma";
    }

    public static boolean temClasse(String nome) {
        try {
            Class.forName(nome, false, Falhas.class.getClassLoader());
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    public static void aoIniciar() {
        throw new IllegalStateException("Warden: falha de teste ao iniciar o cliente (wardenfalhas)");
    }

    public static void aoCriarJogador() {
        String m = modo();
        if (m.equals("entrar")) {
            throw new IllegalStateException("Warden: falha de teste ao entrar no mundo (wardenfalhas)");
        }
        if (m.startsWith("par:") && temClasse(m.substring(4))) {
            throw new IllegalStateException("Warden: conflito de teste com " + m.substring(4) + " (wardenfalhas)");
        }
        if (m.equals("travar")) {
            while (true) {
                try {
                    Thread.sleep(1000);
                } catch (InterruptedException e) {
                    // continua travado de propósito
                }
            }
        }
    }
}
