import { createFileRoute } from '@tanstack/react-router';

import { CreatePackWizard } from '../../features/packs/components/CreatePackWizard';

/** Criar pack (T03). */
export const Route = createFileRoute('/packs/novo')({
  component: CreatePackWizard,
});
