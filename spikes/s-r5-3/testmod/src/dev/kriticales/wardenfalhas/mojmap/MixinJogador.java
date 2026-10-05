package dev.kriticales.wardenfalhas.mojmap;

import dev.kriticales.wardenfalhas.Falhas;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(targets = "net.minecraft.server.level.ServerPlayer", remap = false)
public abstract class MixinJogador {
    @Inject(method = "<init>", at = @At("RETURN"), remap = false)
    private void warden$jogador(CallbackInfo ci) { Falhas.aoCriarJogador(); }
}
