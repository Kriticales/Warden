/**
 * Contador da barra de um painel (esboço do bug-01): só o número à vista, a frase inteira ("1
 * erro", "2 causas") para o leitor de tela e na dica.
 */
export function Count({
  value,
  label,
  kind,
}: {
  value: number;
  label: string;
  kind?: 'danger' | 'warn';
}) {
  return (
    <span className={kind ? `count count--${kind}` : 'count'} title={label}>
      <span aria-hidden="true">{value}</span>
      <span className="sr-only">{label}</span>
    </span>
  );
}
