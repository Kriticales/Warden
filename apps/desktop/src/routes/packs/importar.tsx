import { createFileRoute, Navigate } from '@tanstack/react-router';

import { ImportPage } from '../../features/packs/import/ImportPage';

/** Busca da rota: o arquivo ou a pasta escolhida em "Abrir ou importar…". */
export interface ImportSearch {
  origem: string;
}

/**
 * Importar modpack (SPEC T24): página do nível do app, com "← Meus packs". `origem` é o
 * `.mrpack`, o zip ou a pasta de instância escolhida; sem ela, volta para Meus packs.
 */
export const Route = createFileRoute('/packs/importar')({
  validateSearch: (search: Record<string, unknown>): ImportSearch => ({
    origem: typeof search.origem === 'string' ? search.origem : '',
  }),
  component: ImportRoute,
});

function ImportRoute() {
  const { origem } = Route.useSearch();
  if (origem === '') return <Navigate to="/packs" />;
  // A chave remonta a página quando a pessoa escolhe outro arquivo (plano novo).
  return <ImportPage key={origem} origin={origem} />;
}
