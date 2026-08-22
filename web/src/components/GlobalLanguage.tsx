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

const COPY_BY_LANGUAGE: Record<string, GlobalCopy> = {
  en: ENGLISH_COPY,
  pt: PORTUGUESE_COPY,
  es: {
    chooseLanguage: "Elegir idioma",
    originalLanguage: "Idioma original",
    home: "Inicio",
    generateArticle: "Generar nuevo artículo",
    rssFeed: "Fuente RSS",
    signIn: "Iniciar sesión",
    signOut: "Cerrar sesión",
    siteDescription: "Consulta las últimas noticias con un toque de inestabilidad en The Wibble, tu fuente para el impredecible mundo de la actualidad.",
    promptDetails: "Cómo se solicitó esta imagen",
    viewImageDetails: "Ver cómo se generó esta imagen",
    promptMeaning: "El prompt es la instrucción dada al generador de imágenes que describe lo que debe crear.",
    negativePromptMeaning: "El prompt negativo enumera los detalles que el generador de imágenes debe evitar.",
    model: "Modelo",
    seed: "Semilla",
  },
  fr: {
    chooseLanguage: "Choisir la langue",
    originalLanguage: "Langue d’origine",
    home: "Accueil",
    generateArticle: "Générer un nouvel article",
    rssFeed: "Flux RSS",
    signIn: "Se connecter",
    signOut: "Se déconnecter",
    siteDescription: "Découvrez les dernières actualités avec une touche d’instabilité sur The Wibble, votre source pour le monde imprévisible de l’actualité.",
    promptDetails: "Comment cette image a été demandée",
    viewImageDetails: "Voir comment cette image a été générée",
    promptMeaning: "Le prompt est l’instruction donnée au générateur d’images décrivant ce qu’il doit créer.",
    negativePromptMeaning: "Le prompt négatif énumère les détails que le générateur d’images doit éviter.",
    model: "Modèle",
    seed: "Graine",
  },
  de: {
    chooseLanguage: "Sprache wählen",
    originalLanguage: "Originalsprache",
    home: "Startseite",
    generateArticle: "Neuen Artikel erstellen",
    rssFeed: "RSS-Feed",
    signIn: "Anmelden",
    signOut: "Abmelden",
    siteDescription: "Die neuesten Nachrichten mit einem Hauch von Unbeständigkeit bei The Wibble – deiner Quelle für die unvorhersehbare Welt des aktuellen Geschehens.",
    promptDetails: "Wie dieses Bild angefordert wurde",
    viewImageDetails: "Anzeigen, wie dieses Bild erzeugt wurde",
    promptMeaning: "Der Prompt ist die Anweisung an den Bildgenerator und beschreibt, was er erstellen soll.",
    negativePromptMeaning: "Der negative Prompt listet Details auf, die der Bildgenerator vermeiden soll.",
    model: "Modell",
    seed: "Seed",
  },
  it: {
    chooseLanguage: "Scegli la lingua",
    originalLanguage: "Lingua originale",
    home: "Home",
    generateArticle: "Genera un nuovo articolo",
    rssFeed: "Feed RSS",
    signIn: "Accedi",
    signOut: "Esci",
    siteDescription: "Scopri le ultime notizie con un tocco di instabilità su The Wibble, la tua fonte per l’imprevedibile mondo dell’attualità.",
    promptDetails: "Come è stata richiesta questa immagine",
    viewImageDetails: "Vedi come è stata generata questa immagine",
    promptMeaning: "Il prompt è l’istruzione data al generatore di immagini che descrive ciò che deve creare.",
    negativePromptMeaning: "Il prompt negativo elenca i dettagli che il generatore di immagini deve evitare.",
    model: "Modello",
    seed: "Seme",
  },
  uk: {
    chooseLanguage: "Вибрати мову",
    originalLanguage: "Мова оригіналу",
    home: "Головна",
    generateArticle: "Створити нову статтю",
    rssFeed: "RSS-стрічка",
    signIn: "Увійти",
    signOut: "Вийти",
    siteDescription: "Читайте найсвіжіші новини з ноткою нестабільності на The Wibble — вашому джерелі про непередбачуваний світ поточних подій.",
    promptDetails: "Як було запитано це зображення",
    viewImageDetails: "Переглянути, як було створено це зображення",
    promptMeaning: "Промпт — це інструкція для генератора зображень, яка описує, що саме він має створити.",
    negativePromptMeaning: "Негативний промпт перелічує деталі, яких генератор зображень має уникати.",
    model: "Модель",
    seed: "Сід",
  },
  pl: {
    chooseLanguage: "Wybierz język",
    originalLanguage: "Język oryginalny",
    home: "Strona główna",
    generateArticle: "Wygeneruj nowy artykuł",
    rssFeed: "Kanał RSS",
    signIn: "Zaloguj się",
    signOut: "Wyloguj się",
    siteDescription: "Poznaj najnowsze wiadomości z nutą niestabilności w The Wibble — twoim źródle informacji o nieprzewidywalnym świecie bieżących wydarzeń.",
    promptDetails: "Jak poproszono o ten obraz",
    viewImageDetails: "Zobacz, jak wygenerowano ten obraz",
    promptMeaning: "Prompt to instrukcja przekazana generatorowi obrazów, opisująca, co ma utworzyć.",
    negativePromptMeaning: "Negatywny prompt wymienia szczegóły, których generator obrazów powinien unikać.",
    model: "Model",
    seed: "Ziarno",
  },
  ja: {
    chooseLanguage: "言語を選択",
    originalLanguage: "元の言語",
    home: "ホーム",
    generateArticle: "新しい記事を生成",
    rssFeed: "RSSフィード",
    signIn: "ログイン",
    signOut: "ログアウト",
    siteDescription: "The Wibbleで、少し不安定で予測不能な時事の世界から最新ニュースをお届けします。",
    promptDetails: "この画像への指示内容",
    viewImageDetails: "この画像の生成方法を見る",
    promptMeaning: "プロンプトは、何を作成するかを画像生成器に伝える指示です。",
    negativePromptMeaning: "ネガティブプロンプトは、画像生成器が避けるべき要素を列挙します。",
    model: "モデル",
    seed: "シード",
  },
  zh: {
    chooseLanguage: "选择语言",
    originalLanguage: "原始语言",
    home: "首页",
    generateArticle: "生成新文章",
    rssFeed: "RSS 订阅",
    signIn: "登录",
    signOut: "退出登录",
    siteDescription: "在 The Wibble 获取带有一丝不稳定气息的最新新闻，了解变幻莫测的时事世界。",
    promptDetails: "这张图片的生成要求",
    viewImageDetails: "查看这张图片的生成方式",
    promptMeaning: "提示词是提供给图像生成器的指令，用于描述它应创建的内容。",
    negativePromptMeaning: "负面提示词列出了图像生成器应避免的细节。",
    model: "模型",
    seed: "种子",
  },
  ar: {
    chooseLanguage: "اختيار اللغة",
    originalLanguage: "اللغة الأصلية",
    home: "الرئيسية",
    generateArticle: "إنشاء مقال جديد",
    rssFeed: "خلاصة RSS",
    signIn: "تسجيل الدخول",
    signOut: "تسجيل الخروج",
    siteDescription: "تابع أحدث الأخبار بلمسة من التقلب على The Wibble، مصدرك لعالم الأحداث الجارية غير المتوقع.",
    promptDetails: "كيف طُلب إنشاء هذه الصورة",
    viewImageDetails: "عرض كيفية إنشاء هذه الصورة",
    promptMeaning: "الموجّه هو التعليمات المقدمة إلى مولّد الصور لوصف ما ينبغي له إنشاؤه.",
    negativePromptMeaning: "يسرد الموجّه السلبي التفاصيل التي ينبغي لمولّد الصور تجنبها.",
    model: "النموذج",
    seed: "البذرة",
  },
  hi: {
    chooseLanguage: "भाषा चुनें",
    originalLanguage: "मूल भाषा",
    home: "होम",
    generateArticle: "नया लेख बनाएँ",
    rssFeed: "RSS फ़ीड",
    signIn: "साइन इन करें",
    signOut: "साइन आउट करें",
    siteDescription: "The Wibble पर अस्थिरता के एक स्पर्श के साथ नवीनतम समाचार पाएँ—वर्तमान घटनाओं की अप्रत्याशित दुनिया के लिए आपका स्रोत।",
    promptDetails: "इस चित्र के लिए क्या निर्देश दिया गया",
    viewImageDetails: "देखें कि यह चित्र कैसे बनाया गया",
    promptMeaning: "प्रॉम्प्ट चित्र जनरेटर को दिया गया निर्देश है, जो बताता है कि उसे क्या बनाना चाहिए।",
    negativePromptMeaning: "नकारात्मक प्रॉम्प्ट उन विवरणों को सूचीबद्ध करता है जिनसे चित्र जनरेटर को बचना चाहिए।",
    model: "मॉडल",
    seed: "सीड",
  },
};

export const globalCopyForLanguage = (language: string | null): GlobalCopy => {
  if (!language) return ENGLISH_COPY;
  try {
    const locale = new Intl.Locale(language);
    return COPY_BY_LANGUAGE[locale.language] ?? ENGLISH_COPY;
  } catch {
    return ENGLISH_COPY;
  }
};

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
