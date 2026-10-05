/**
 * Navegação do nível do app (ESTRUTURA; ADR-0026): sem barra lateral. A barra do app mostra,
 * à direita, os destinos registrados aqui (por exemplo, Configurações).
 *
 * Registro acréscimo-apenas (ROADMAP §1): a tarefa dona de cada página acrescenta uma linha
 * em `appBarLinks` quando a rota existir (a P1-13 acrescenta Configurações). Nenhum link
 * aparece antes de a página existir.
 */
import type { LinkProps } from '@tanstack/react-router';
import { Settings, type LucideIcon } from 'lucide-react';

/** Um destino da barra do app. */
export interface AppBarLink {
  /** Rota (tipada pelo TanStack Router). */
  to: NonNullable<LinkProps['to']>;
  /** Chave do texto em `navegacao.ts`. */
  labelKey: 'configuracoes' | 'meusPacks';
  icon: LucideIcon;
}

export const appBarLinks: readonly AppBarLink[] = [
  { to: '/configuracoes', labelKey: 'configuracoes', icon: Settings },
];
