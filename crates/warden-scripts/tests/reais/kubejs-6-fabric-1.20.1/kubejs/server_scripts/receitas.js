// Receitas do Vale Sereno
ServerEvents.recipes(event => {
  // Bloco de cobre com lingotes
  event.shaped('minecraft:copper_block', [
    'CCC',
    'CCC',
    'CCC'
  ], {
    C: 'minecraft:copper_ingot'
  })

  // Para-raios com cobre e graveto
  event.shapeless('minecraft:lightning_rod', ['minecraft:copper_ingot', 'minecraft:stick'])

  // Lingotes de volta (com erro de digitação)
  event.shapeless('9x minecraft:copper_ingott', ['minecraft:copper_block'])

  event.shapless('minecraft:stick', ['minecraft:oak_planks'])
})
