package dev.kriticales.wardenfalhas.mojmap;

import dev.kriticales.wardenfalhas.Falhas;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(targets = "net.minecraft.client.Minecraft", remap = false)
public abstract class MixinIniciar {
    @Inject(method = "<init>", at = @At("RETURN"), remap = false)
    private void warden$iniciar(CallbackInfo ci) { Falhas.aoIniciar(); }
}
