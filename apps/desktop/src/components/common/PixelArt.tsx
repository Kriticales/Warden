/**
 * Pixel art própria do Warden, desenhada por código (DESIGN-SYSTEM §3.7 e §9): a marca e as
 * ilustrações dos estados vazios. Portadas de `design/system/components.js` (`brandMark`,
 * `art`). Exceção documentada à regra dos tokens: as cores ficam escritas no desenho.
 */

/** Um retângulo: x, y, largura, altura, cor e opacidade opcional. */
type Pixel = readonly [number, number, number, number, string, string?];

function Pixels({ pixels }: { pixels: readonly Pixel[] }) {
  return (
    <>
      {pixels.map(([x, y, width, height, fill, opacity], index) => (
        <rect
          // A lista é fixa: a posição identifica o retângulo.
          key={index}
          x={x}
          y={y}
          width={width}
          height={height}
          fill={fill}
          fillOpacity={opacity}
        />
      ))}
    </>
  );
}

const BRAND: readonly Pixel[] = [
  [2, 5, 12, 11, '#010507'],
  [3, 6, 10, 9, '#0e2630'],
  [3, 6, 10, 1, '#1e4552'],
  [3, 14, 10, 1, '#0a1a21'],
  [4, 1, 2, 5, '#19d3e0'],
  [10, 1, 2, 5, '#19d3e0'],
  [4, 0, 2, 1, '#7ff3f0'],
  [10, 0, 2, 1, '#7ff3f0'],
  [3, 2, 1, 1, '#0fb5c2'],
  [12, 2, 1, 1, '#0fb5c2'],
  [7, 8, 2, 5, '#19d3e0'],
  [6, 9, 4, 3, '#19d3e0'],
  [7, 9, 2, 2, '#7ff3f0'],
  [4, 12, 1, 1, '#19d3e0', '.6'],
  [11, 8, 1, 1, '#19d3e0', '.5'],
  [11, 13, 1, 1, '#7ff3f0', '.5'],
];

/** Marca do Warden: bloco escuro com duas antenas acesas e um núcleo de alma (16×16). */
export function BrandMark({ className = 'brand__mark' }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 16 16"
      shapeRendering="crispEdges"
      aria-hidden="true"
      focusable="false"
    >
      <Pixels pixels={BRAND} />
    </svg>
  );
}

/** Símbolos das ilustrações (6×6, "X" = pixel aceso). */
export const GLYPHS = {
  plus: ['..XX..', '..XX..', 'XXXXXX', 'XXXXXX', '..XX..', '..XX..'],
  check: ['.....X', '....XX', 'X..XX.', 'XXXX..', '.XX...', '......'],
  x: ['XX..XX', '.XXXX.', '..XX..', '..XX..', '.XXXX.', 'XX..XX'],
  search: ['.XXX..', 'X...X.', 'X...X.', '.XXXX.', '....XX', '.....X'],
  dots: ['......', '......', 'XX.XX.', 'XX.XX.', '......', '......'],
  box: ['XXXXXX', 'X....X', 'X.XX.X', 'X.XX.X', 'X....X', 'XXXXXX'],
  clock: ['.XXXX.', 'X..X.X', 'X..X.X', 'X..XXX', 'X....X', '.XXXX.'],
  link: ['XXX...', 'X.X...', 'XXXXXX', '...X.X', '...XXX', '......'],
} as const;

export type Glyph = keyof typeof GLYPHS;

/** Cor do símbolo: ciano (padrão) ou a de um estado. */
export type ArtTone = 'primary' | 'ok' | 'danger' | 'warn' | 'muted' | 'ai';

const TONE_COLOR: Record<ArtTone, string> = {
  primary: '#19d3e0',
  ok: '#3ddc84',
  danger: '#ff5c5c',
  warn: '#f2b33d',
  muted: '#8aacaa',
  ai: '#cfe3da',
};

const BLOCK: readonly Pixel[] = [
  [2, 3, 14, 14, '#010507'],
  [3, 4, 12, 3, '#1e4552'],
  [3, 7, 12, 9, '#0e2630'],
  [3, 15, 12, 1, '#0a1a21'],
  [4, 4, 3, 1, '#2b5a68'],
  [10, 5, 2, 1, '#2b5a68'],
  [3, 13, 2, 1, '#19d3e0'],
  [13, 9, 2, 1, '#0fb5c2'],
  [12, 15, 2, 1, '#19d3e0'],
];

/** Pixels acesos de um símbolo, já na posição dentro do bloco. */
export function glyphPixels(glyph: Glyph, color: string): Pixel[] {
  const pixels: Pixel[] = [];
  GLYPHS[glyph].forEach((row, y) => {
    for (let x = 0; x < row.length; x += 1) {
      if (row[x] === 'X') pixels.push([6 + x, 8 + y, 1, 1, color]);
    }
  });
  return pixels;
}

/** Ilustração de estado vazio: um bloco de frente com um símbolo em pixel (18×18). */
export function EmptyArt({
  glyph = 'box',
  tone = 'primary',
  className = 'empty__art',
}: {
  glyph?: Glyph;
  tone?: ArtTone;
  className?: string | undefined;
}) {
  return (
    <svg
      className={className}
      viewBox="0 0 18 18"
      shapeRendering="crispEdges"
      aria-hidden="true"
      focusable="false"
    >
      <Pixels pixels={BLOCK} />
      <Pixels pixels={glyphPixels(glyph, TONE_COLOR[tone])} />
    </svg>
  );
}

/** FNV-1a de 32 bits, igual ao `hash()` de `components.js`. */
function hash(text: string): number {
  let h = 2166136261;
  for (const char of text) {
    h = Math.imul(h ^ char.charCodeAt(0), 16777619);
  }
  return h >>> 0;
}

/**
 * Ícone provisório 8×8, simétrico, gerado do nome (`tileSvg()` de `components.js`; HANDOFF:
 * "Mod sem ícone na API"). O mesmo nome dá sempre o mesmo desenho.
 */
export function tilePixels(seed: string): Pixel[] {
  const h = hash(seed);
  const hue = h % 360;
  const fg = `hsl(${String(hue)} 46% 52%)`;
  const hi = `hsl(${String((hue + 30) % 360)} 60% 72%)`;
  const pixels: Pixel[] = [[0, 0, 8, 8, `hsl(${String(hue)} 34% 18%)`]];
  let bits = h;
  for (let y = 1; y < 7; y += 1) {
    for (let x = 1; x < 4; x += 1) {
      bits = Math.imul(bits ^ (bits >>> 13), 0x5bd1e995) >>> 0;
      const v = bits % 5;
      if (v < 2) continue;
      const color = v === 4 ? hi : fg;
      pixels.push([x, y, 1, 1, color], [7 - x, y, 1, 1, color]);
    }
  }
  return pixels;
}

/** Tamanho do ícone: 32 px (padrão), 24, 40 ou 56 px. */
export type TileSize = 'sm' | 'md' | 'lg' | 'xl';

/** Ícone de pack ou mod sem imagem própria: o desenho gerado do nome, decorativo. */
export function NameTile({ seed, size = 'md' }: { seed: string; size?: TileSize }) {
  return (
    <span className={size === 'md' ? 'tile' : `tile tile--${size}`} aria-hidden="true">
      <svg viewBox="0 0 8 8" shapeRendering="crispEdges" focusable="false">
        <Pixels pixels={tilePixels(seed)} />
      </svg>
    </span>
  );
}
