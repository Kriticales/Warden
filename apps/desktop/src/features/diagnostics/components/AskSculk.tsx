/**
 * As ligações de Problemas com o Sensor Sculk (esboço aprovado do bug-01, "Separadas"): cada
 * achado, travamento e o desempenho do último teste têm um botão que abre uma conversa nova do
 * Sculk já com aquilo como assunto. É sempre um link para a seção (`/packs/$packId/ia`), com a
 * sessão do teste (`sessao`), o sintoma (`sintoma`) e o texto do assunto (`assunto`, que vai
 * para "Contar algo"); lá o consentimento abre sozinho e nada é enviado antes de começar.
 *
 * O travamento que já tem conversa ligada (pela assinatura) abre essa conversa em vez de uma
 * nova.
 */
import { Link } from '@tanstack/react-router';
import { MessageSquare, Sparkles } from 'lucide-react';
import type { ReactNode } from 'react';

import { Button } from '../../../components/ui/button';
import type { AiSymptom, PackId } from '../../../lib/ipc/bindings';

export interface SculkSubject {
  /** A sessão do Testar que a conversa analisa. */
  sessao?: string;
  sintoma?: AiSymptom;
  /** O texto que vai para "Contar algo". */
  assunto?: string;
}

/** "✦ …" que abre uma conversa nova com o assunto. */
export function AskSculkLink({
  packId,
  subject,
  label,
  children,
}: {
  packId: PackId;
  subject: SculkSubject;
  /** Rótulo completo para o leitor de tela (o texto do botão é curto). */
  label?: string;
  children: ReactNode;
}) {
  return (
    <Button size="sm" variant="ai" icon={Sparkles} asChild>
      <Link to="/packs/$packId/ia" params={{ packId }} search={subject} aria-label={label}>
        {children}
      </Link>
    </Button>
  );
}

/** "Abrir a conversa com o Sculk": a conversa que já existe. */
export function OpenSculkConversation({
  packId,
  conversationId,
  label,
  children,
}: {
  packId: PackId;
  conversationId: string;
  label?: string;
  children: ReactNode;
}) {
  return (
    <Button size="sm" variant="ai" icon={MessageSquare} asChild>
      <Link
        to="/packs/$packId/ia/$conversaId"
        params={{ packId, conversaId: conversationId }}
        aria-label={label}
      >
        {children}
      </Link>
    </Button>
  );
}
