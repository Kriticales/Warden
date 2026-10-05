/**
 * Página do nível do app: barra do app em cima e o conteúdo com rolagem própria (o rodapé é
 * da raiz). Landmarks: um `<header>` (a barra) e um `<main id="conteudo">` por tela
 * (HANDOFF §7). O título da página (`PageTitle`) recebe o foco quando a rota muda.
 */
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { MAIN_ID } from './focus';
import { TopBar, type TopBarProps } from './TopBar';

export interface AppPageProps extends TopBarProps {
  children: ReactNode;
  /** Conteúdo estreito (até 760 px), para formulários e textos. */
  narrow?: boolean;
}

export function AppPage({ children, narrow = false, ...topBar }: AppPageProps) {
  return (
    <>
      <TopBar {...topBar} />
      <div className="app__body">
        <main className={cn('app__main', narrow && 'app__main--narrow')} id={MAIN_ID} tabIndex={-1}>
          <div className="content">{children}</div>
        </main>
      </div>
    </>
  );
}
