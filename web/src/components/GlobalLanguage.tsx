import { useRouter } from "next/router";
import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { resolveSupportedTranslationLanguage } from "@/core/translationLanguages";

export type GlobalCopy = {
  chooseLanguage: string;
  originalLanguage: string;
  home: string;
  generateArticle: string;
  rssFeed: string;
  signIn: string;
  signOut: string;
  siteDescription: string;
  promptDetails: string;
  viewImageDetails: string;
  promptMeaning: string;
  negativePromptMeaning: string;
  model: string;
  seed: string;
};

const ENGLISH_COPY: GlobalCopy = {
  chooseLanguage: "Choose language",
  originalLanguage: "Original language",
  home: "Home",
  generateArticle: "Generate new article",
  rssFeed: "RSS Feed",
  signIn: "Sign in",
  signOut: "Sign out",
  siteDescription: "Get the latest news with a touch of wobble from The Wibble, your source for the unpredictable and unsteady world of current events.",
  promptDetails: "How this image was requested",
  viewImageDetails: "View how this image was generated",
  promptMeaning: "The prompt is the instruction given to the image generator describing what it should create.",
  negativePromptMeaning: "The negative prompt lists details the image generator should avoid.",
  model: "Model",
  seed: "Seed",
};

const PORTUGUESE_COPY: GlobalCopy = {
  chooseLanguage: "Escolher idioma",
  originalLanguage: "Idioma original",
  home: "Início",
  generateArticle: "Gerar novo artigo",
  rssFeed: "Feed RSS",
  signIn: "Entrar",
  signOut: "Sair",
  siteDescription: "Veja as notícias mais recentes com um toque de instabilidade no The Wibble, sua fonte para o mundo imprevisível dos acontecimentos atuais.",
  promptDetails: "Como esta imagem foi solicitada",
  viewImageDetails: "Ver como esta imagem foi gerada",
  promptMeaning: "O prompt é a instrução fornecida ao gerador de imagens descrevendo o que ele deve criar.",
  negativePromptMeaning: "O prompt negativo lista detalhes que o gerador de imagens deve evitar.",
  model: "Modelo",
  seed: "Semente",
};

export const globalCopyForLanguage = (language: string | null): GlobalCopy =>
  language?.startsWith("pt") ? PORTUGUESE_COPY : ENGLISH_COPY;

const STORAGE_KEY = "wibble-global-language-v1";

type GlobalLanguageValue = {
  language: string | null;
  copy: GlobalCopy;
  setLanguage: (language: string | null) => Promise<void>;
};

const GlobalLanguageContext = createContext<GlobalLanguageValue>({
  language: null,
  copy: ENGLISH_COPY,
  setLanguage: async () => undefined,
});

const languageFromQuery = (value: string | string[] | undefined): string | null => {
  if (typeof value !== "string") return null;
  try {
    return resolveSupportedTranslationLanguage(value);
  } catch {
    return null;
  }
};

export const GlobalLanguageProvider = ({ children }: { children: React.ReactNode }) => {
  const router = useRouter();
  const queryLanguage = languageFromQuery(router.query.lang);
  const [language, setLanguageState] = useState<string | null>(queryLanguage);

  const navigate = useCallback(async (nextLanguage: string | null) => {
    const query = { ...router.query };
    delete query.afterId;
    if (nextLanguage) query.lang = nextLanguage;
    else delete query.lang;
    setLanguageState(nextLanguage);
    window.localStorage.setItem(STORAGE_KEY, nextLanguage ?? "original");
    const secure = window.location.protocol === "https:" ? "; Secure" : "";
    document.cookie = `wibble_lang=${encodeURIComponent(nextLanguage ?? "original")}; Path=/; Max-Age=31536000; SameSite=Lax${secure}`;
    await router.replace({ pathname: router.pathname, query }, undefined, { shallow: false });
  }, [router]);

  useEffect(() => {
    if (!router.isReady) return;
    if (queryLanguage) {
      setLanguageState(queryLanguage);
      window.localStorage.setItem(STORAGE_KEY, queryLanguage);
      return;
    }
    if (router.query.lang !== undefined) return;

    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "original") {
      setLanguageState(null);
      return;
    }
    let preferred: string | null = null;
    try {
      preferred = stored
        ? resolveSupportedTranslationLanguage(stored)
        : (navigator.languages ?? [navigator.language]).flatMap((candidate) => {
            try {
              return [resolveSupportedTranslationLanguage(candidate)];
            } catch {
              return [];
            }
          })[0] ?? null;
    } catch {
      preferred = null;
    }
    if (!preferred || preferred === "en") {
      setLanguageState(null);
      return;
    }
    void navigate(preferred);
  }, [navigate, queryLanguage, router.isReady, router.query.lang]);

  useEffect(() => {
    document.documentElement.lang = language ?? "en";
  }, [language]);

  const value = useMemo<GlobalLanguageValue>(() => ({
    language,
    copy: globalCopyForLanguage(language),
    setLanguage: navigate,
  }), [language, navigate]);

  return <GlobalLanguageContext.Provider value={value}>{children}</GlobalLanguageContext.Provider>;
};

export const useGlobalLanguage = () => useContext(GlobalLanguageContext);
