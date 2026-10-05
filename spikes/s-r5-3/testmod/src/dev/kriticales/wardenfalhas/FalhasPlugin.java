package dev.kriticales.wardenfalhas;

import java.util.List;
import java.util.Set;
import org.objectweb.asm.tree.ClassNode;
import org.spongepowered.asm.mixin.extensibility.IMixinConfigPlugin;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;

/** Liga só o mixin do modo escolhido. */
public final class FalhasPlugin implements IMixinConfigPlugin {
    @Override public void onLoad(String mixinPackage) {}
    @Override public String getRefMapperConfig() { return null; }
    @Override public boolean shouldApplyMixin(String targetClassName, String mixinClassName) {
        String m = Falhas.modo();
        if (mixinClassName.endsWith("Quebrado")) return m.equals("mixin");
        if (mixinClassName.endsWith("Iniciar")) return m.equals("iniciar");
        if (mixinClassName.endsWith("Jogador")) return m.equals("entrar") || m.equals("travar") || m.startsWith("par:");
        return false;
    }
    @Override public void acceptTargets(Set<String> myTargets, Set<String> otherTargets) {}
    @Override public List<String> getMixins() { return null; }
    @Override public void preApply(String t, ClassNode c, String m, IMixinInfo i) {}
    @Override public void postApply(String t, ClassNode c, String m, IMixinInfo i) {}
}
