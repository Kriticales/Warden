import { createFileRoute } from '@tanstack/react-router';

import { OnboardingWizard } from '../features/onboarding/components/OnboardingWizard';

/** Primeira execução (T01); reaberta por Configurações → "Rever boas-vindas". */
export const Route = createFileRoute('/boas-vindas')({
  component: OnboardingWizard,
});
