package dev.kriticales.wardenfalhas.fabric;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Alvo inexistente: simula um mod feito para outra versão do jogo ou do mod que ele altera. */
@Mixin(targets = "net.minecraft.class_310", remap = false)
public abstract class MixinQuebrado {
    @Inject(method = "wardenMetodoQueNaoExiste", at = @At("HEAD"), remap = false, require = 1)
    private void warden$quebrado(CallbackInfo ci) {}
}
