/**
 * Mini-gráfico em blocos da memória do jogo (protótipo `W.blockChart`; design system `.bchart`):
 * uma coluna por valor, com a altura em blocos proporcional ao máximo, e as colunas acima do
 * limite de aviso em amarelo. É uma imagem para o leitor de tela: o rótulo diz o que ela mostra.
 */
const ROWS = 5;

export interface BlockChartProps {
  /** Os valores, do mais velho para o mais novo. */
  values: readonly number[];
  /** O valor que enche a coluna. */
  max: number;
  /** A partir daqui a coluna fica em aviso. */
  warnAt: number;
  /** O que o gráfico mostra, em texto. */
  label: string;
}

export function BlockChart({ values, max, warnAt, label }: BlockChartProps) {
  return (
    <span className="bchart" role="img" aria-label={label}>
      {values.map((value, index) => {
        const height = Math.min(ROWS, Math.max(1, Math.round((value / Math.max(max, 1)) * ROWS)));
        return (
          // A posição é a identidade da coluna (o gráfico só anda para a esquerda).
          <span key={index} className={value >= warnAt ? 'bchart__col is-hot' : 'bchart__col'}>
            {Array.from({ length: ROWS }, (_, row) => (
              <i key={row} className={ROWS - row <= height ? 'on' : undefined} />
            ))}
          </span>
        );
      })}
    </span>
  );
}
