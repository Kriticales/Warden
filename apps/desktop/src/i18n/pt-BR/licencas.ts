/**
 * Licenças de terceiros e créditos (A-02; SPEC T23; ADR-0004), abertos a partir de
 * "Sobre o Warden".
 */
export const licencas = {
  abrir: 'Licenças de terceiros',
  titulo: 'Licenças de terceiros',
  descricao:
    'O Warden é feito com {{count}} componentes de código aberto. Aqui estão as licenças de cada um.',
  carregando: 'Lendo as licenças…',
  naoGeradaTitulo: 'Lista de licenças indisponível',
  naoGeradaTexto:
    'Esta compilação do Warden foi feita sem a lista de licenças. Para incluir, rode "cargo xtask notices" antes de compilar.',
  creditosTitulo: 'Créditos',
  creditoForge:
    'Forge: o Warden baixa o Forge sempre do Maven oficial e o instala com o instalador do próprio Forge. O Forge é feito por LexManos e colaboradores, que pedem apoio ao projeto.',
  creditoPackwiz:
    'packwiz: o formato e a ferramenta de modpacks usados pelo Warden, de comp500 e colaboradores. Vai embutido no app.',
  creditoPortablemc:
    'portablemc: o motor que instala e abre o Minecraft nos testes, de Théo Rozier e colaboradores.',
  filtro: 'Filtrar por nome ou licença',
  semResultado: 'Nenhum componente com "{{filtro}}".',
  grupoContagem: '{{count}} componentes',
  grupos: {
    rust: 'Rust (o app)',
    npm: 'JavaScript (a interface)',
    packwiz: 'packwiz embutido',
    fontes: 'Fontes',
  },
  licencaNaoIdentificada: 'Licença não identificada: veja o texto',
  verLicenca: 'Ver a licença',
  semTexto: 'O pacote não traz o texto da licença.',
  fechar: 'Fechar',
} as const;
