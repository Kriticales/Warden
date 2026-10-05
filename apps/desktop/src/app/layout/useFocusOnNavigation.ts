/**
 * Ao trocar de tela, o foco vai para o título da nova página (HANDOFF §7), para o leitor de
 * tela anunciar onde se está. A primeira tela do app não rouba o foco.
 */
import { useRouter } from '@tanstack/react-router';
import { useEffect } from 'react';

import { focusPageTitle } from './focus';

export function useFocusOnNavigation(): void {
  const router = useRouter();
  useEffect(
    () =>
      router.subscribe('onResolved', (event) => {
        if (event.fromLocation && event.pathChanged) {
          focusPageTitle();
        }
      }),
    [router],
  );
}
