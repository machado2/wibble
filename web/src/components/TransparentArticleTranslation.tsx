import { useSession } from "next-auth/react";
import { useEffect, useState } from "react";
import { useGlobalLanguage } from "./GlobalLanguage";

const inFlight = new Map<string, Promise<void>>();
const completed = new Set<string>();

type Props = {
  slug: string;
  requestedLanguage?: string;
  currentLanguage: string | null;
  availableLanguages: string[];
  onReady: () => Promise<void>;
};

export const TransparentArticleTranslation = ({
  slug,
  requestedLanguage,
  currentLanguage,
  availableLanguages,
  onReady,
}: Props) => {
  const { data: session, status } = useSession();
  const { copy } = useGlobalLanguage();
  const [message, setMessage] = useState<string | null>(null);
  const email = session?.user?.email?.toLowerCase();

  useEffect(() => {
    if (!requestedLanguage || currentLanguage === requestedLanguage) return;
    if (status !== "authenticated" || !email) {
      if (status === "unauthenticated") setMessage(copy.signInToTranslate);
      return;
    }

    const key = `${email}:${slug}:${requestedLanguage}`;
    if (completed.has(key)) return;
    let request = inFlight.get(key);
    if (!request) {
      setMessage(availableLanguages.includes(requestedLanguage)
        ? copy.loadingTranslation
        : copy.translatingArticle);
      request = (async () => {
        const response = await fetch(`/api/translations/${encodeURIComponent(slug)}`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ language: requestedLanguage }),
        });
        await response.json().catch(() => null);
        if (!response.ok) throw new Error(copy.translationGenerationError);
        completed.add(key);
        await onReady();
      })();
      inFlight.set(key, request);
      request.finally(() => inFlight.delete(key)).catch(() => undefined);
    }

    let cancelled = false;
    request.catch((error) => {
      if (!cancelled) setMessage(error instanceof Error ? error.message : copy.translationFailed);
    });
    return () => { cancelled = true; };
  }, [availableLanguages, copy, currentLanguage, email, onReady, requestedLanguage, slug, status]);

  return message ? <small role="status">{message}</small> : null;
};
