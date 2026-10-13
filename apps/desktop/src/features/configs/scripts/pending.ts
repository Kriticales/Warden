/**
 * A linha a mostrar quando um erro de outro script é clicado: o painel troca de arquivo pela
 * URL e o editor novo vai à linha assim que abre.
 */
let pending: { path: string; line: number; column: number } | null = null;

export function setPendingLine(path: string, line: number, column = 1): void {
  pending = { path: path.toLowerCase(), line, column };
}

/** A linha guardada para o arquivo (só uma vez). */
export function takePendingLine(path: string): { line: number; column: number } | null {
  if (pending?.path !== path.toLowerCase()) {
    return null;
  }
  const found = { line: pending.line, column: pending.column };
  pending = null;
  return found;
}
