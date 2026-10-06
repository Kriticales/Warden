import { createFileRoute } from '@tanstack/react-router';

import { PacksPage } from '../../features/packs/components/PacksPage';

/** Meus packs (T02): a tela inicial do app. */
export const Route = createFileRoute('/packs/')({
  component: PacksPage,
});
