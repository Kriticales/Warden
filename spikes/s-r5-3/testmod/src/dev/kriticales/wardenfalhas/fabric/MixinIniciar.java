package dev.kriticales.wardenfalhas.fabric;

import dev.kriticales.wardenfalhas.Falhas;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(targets = "net.minecraft.class_310", remap = false)
public abstract class MixinIniciar {
    @Inject(method = "<init>", at = @At("RETURN"), remap = false)
    private void warden$iniciar(CallbackInfo ci) { Falhas.aoIniciar(); }
}
