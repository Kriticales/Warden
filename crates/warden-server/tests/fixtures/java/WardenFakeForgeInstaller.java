import java.io.File;
import java.io.FileOutputStream;

/**
 * Instalador do Forge simulado dos testes da warden-server (ROADMAP L-09): com
 * `--installServer`, cria o arquivo de argumentos que o Forge 1.17+ deixa
 * (`libraries/net/minecraftforge/forge/1.20.1-47.3.0/{unix,win}_args.txt`) na pasta atual.
 * Com o arquivo `falhar` na pasta, sai com 3; com `nada`, sai com 0 sem instalar (como o
 * instalador real sem rede).
 *
 * Compilado para Java 8: javac --release 8 -d . WardenFakeForgeInstaller.java
 * e empacotado: jar cfe forge-simulado-installer.jar WardenFakeForgeInstaller *.class
 */
public final class WardenFakeForgeInstaller {
    private WardenFakeForgeInstaller() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 0 || !args[0].equals("--installServer")) {
            System.err.println("uso: --installServer");
            System.exit(2);
        }
        if (new File("falhar").exists()) {
            System.out.println("Error: There was an error during installation");
            System.exit(3);
        }
        if (new File("nada").exists()) {
            System.out.println("Failed to validate certificates");
            System.exit(0);
        }
        File dir = new File("libraries/net/minecraftforge/forge/1.20.1-47.3.0");
        dir.mkdirs();
        for (String name : new String[] {"unix_args.txt", "win_args.txt"}) {
            FileOutputStream out = new FileOutputStream(new File(dir, name));
            out.write("-cp x net.minecraftforge.Main\n".getBytes("UTF-8"));
            out.close();
        }
        System.out.println("The server installed successfully");
    }
}
