/**
 * Encaixes de outras tarefas dentro das seções de Configurações (registro acréscimo-apenas,
 * ROADMAP §1). Cada componente desenha só o próprio conteúdo, sem título: o título é da seção
 * que o recebe.
 *
 * - `testSectionSlots`: a tabela de Java (L-01, `java/`), depois de "Memória padrão", como no
 *   protótipo;
 * - `keysSectionSlots`: "Entrar com o GitHub" (UI-03, `github/`) e o modelo do Gemini (D-04,
 *   `ai/`), depois das chaves.
 */
import type { ComponentType } from 'react';

import { GithubLogin } from './github/GithubLogin';

/** Encaixes dentro de "Teste" (L-01: tabela de Java). */
export const testSectionSlots: readonly ComponentType[] = [];

/** Encaixes dentro de "Chaves e contas" (UI-03: login do GitHub; D-04: modelo do Gemini). */
export const keysSectionSlots: readonly ComponentType[] = [GithubLogin];
