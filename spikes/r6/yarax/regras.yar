// Regras de teste do spike R6 (não são a lista do Warden).

// Bytecode que monta o IP do estágio 0 (85.217.144.130) com bipush/bastore.
// Padrão do jar-infection-scanner da Overwolf (MIT), kSignatures[0].
rule fractureiser_ip_bytecode {
  strings:
    $ip = { 38 54 59 04 10 35 54 59 05 10 2E 54 59 06 10 32 54 59 07 10 31 54 59 08 10 37 54 59 10 06 10 2E 54 59 10 07 10 31 54 59 10 08 10 34 54 59 10 09 10 34 54 59 10 0A 10 2E 54 59 10 0B 10 31 54 59 10 0C 10 33 54 59 10 0D 10 30 54 B7 }
  condition:
    $ip
}

// Estrutura do estágio 0: os descritores que o SIG1 usa, todos no pool de constantes da mesma classe.
rule fractureiser_stage0_descritores {
  strings:
    $a = "([Ljava/lang/Class;)Ljava/lang/reflect/Constructor;"
    $b = "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;"
    $c = "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;)V"
    $d = "([Ljava/lang/Object;)Ljava/lang/Object;"
    $e = "java/net/URL"
  condition:
    uint32be(0) == 0xCAFEBABE and all of them
}

rule atencao_webhook_discord {
  strings:
    $w = /discord(app)?\.com\/api\/webhooks\//
  condition:
    $w
}
