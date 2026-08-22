import { useRouter } from "next/router";
import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { resolveSupportedTranslationLanguage } from "@/core/translationLanguages";

type SharedCopy = {
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

type HomeCopy = {
  newsTab: string;
  imagesTab: string;
  sortNew: string;
  sortVotes: string;
  sortViews: string;
  periodWeek: string;
  periodMonth: string;
  periodAll: string;
  searchPlaceholder: string;
  loadingArticles: string;
  noArticles: string;
  nextPage: string;
  upvote: string;
  downvote: string;
  voteSaveError: string;
};

type ExtraCopy = {
  imagesLoadError: string;
  loadingImages: string;
  noImages: string;
  signInToTranslate: string;
  loadingTranslation: string;
  translatingArticle: string;
  translationGenerationError: string;
  translationFailed: string;
  contentLoadError: string;
};

export type GlobalCopy = SharedCopy & HomeCopy & ExtraCopy;

const ENGLISH_COPY: SharedCopy = {
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

const PORTUGUESE_COPY: SharedCopy = {
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

const COPY_BY_LANGUAGE: Record<string, SharedCopy> = {
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

const ENGLISH_HOME_COPY: HomeCopy = {
  newsTab: "News",
  imagesTab: "Images",
  sortNew: "New",
  sortVotes: "Votes",
  sortViews: "Views",
  periodWeek: "Week",
  periodMonth: "Month",
  periodAll: "All",
  searchPlaceholder: "Search",
  loadingArticles: "Loading articles…",
  noArticles: "No articles found.",
  nextPage: "Next page",
  upvote: "Upvote",
  downvote: "Downvote",
  voteSaveError: "Vote could not be saved.",
};

const HOME_COPY_BY_LANGUAGE: Record<string, HomeCopy> = {
  en: ENGLISH_HOME_COPY,
  pt: {
    newsTab: "Notícias",
    imagesTab: "Imagens",
    sortNew: "Novos",
    sortVotes: "Votos",
    sortViews: "Visualizações",
    periodWeek: "Semana",
    periodMonth: "Mês",
    periodAll: "Tudo",
    searchPlaceholder: "Buscar",
    loadingArticles: "Carregando artigos…",
    noArticles: "Nenhum artigo encontrado.",
    nextPage: "Próxima página",
    upvote: "Votar a favor",
    downvote: "Votar contra",
    voteSaveError: "Não foi possível salvar o voto.",
  },
  es: {
    newsTab: "Noticias", imagesTab: "Imágenes", sortNew: "Nuevos", sortVotes: "Votos",
    sortViews: "Visualizaciones", periodWeek: "Semana", periodMonth: "Mes", periodAll: "Todo",
    searchPlaceholder: "Buscar", loadingArticles: "Cargando artículos…", noArticles: "No se encontraron artículos.",
    nextPage: "Página siguiente", upvote: "Votar a favor", downvote: "Votar en contra", voteSaveError: "No se pudo guardar el voto.",
  },
  fr: {
    newsTab: "Actualités", imagesTab: "Images", sortNew: "Nouveaux", sortVotes: "Votes",
    sortViews: "Vues", periodWeek: "Semaine", periodMonth: "Mois", periodAll: "Tout",
    searchPlaceholder: "Rechercher", loadingArticles: "Chargement des articles…", noArticles: "Aucun article trouvé.",
    nextPage: "Page suivante", upvote: "Vote positif", downvote: "Vote négatif", voteSaveError: "Le vote n’a pas pu être enregistré.",
  },
  de: {
    newsTab: "Nachrichten", imagesTab: "Bilder", sortNew: "Neu", sortVotes: "Stimmen",
    sortViews: "Aufrufe", periodWeek: "Woche", periodMonth: "Monat", periodAll: "Alle",
    searchPlaceholder: "Suchen", loadingArticles: "Artikel werden geladen…", noArticles: "Keine Artikel gefunden.",
    nextPage: "Nächste Seite", upvote: "Positiv bewerten", downvote: "Negativ bewerten", voteSaveError: "Die Stimme konnte nicht gespeichert werden.",
  },
  it: {
    newsTab: "Notizie", imagesTab: "Immagini", sortNew: "Nuovi", sortVotes: "Voti",
    sortViews: "Visualizzazioni", periodWeek: "Settimana", periodMonth: "Mese", periodAll: "Tutto",
    searchPlaceholder: "Cerca", loadingArticles: "Caricamento articoli…", noArticles: "Nessun articolo trovato.",
    nextPage: "Pagina successiva", upvote: "Voto positivo", downvote: "Voto negativo", voteSaveError: "Impossibile salvare il voto.",
  },
  uk: {
    newsTab: "Новини", imagesTab: "Зображення", sortNew: "Нові", sortVotes: "Голоси",
    sortViews: "Перегляди", periodWeek: "Тиждень", periodMonth: "Місяць", periodAll: "Усе",
    searchPlaceholder: "Пошук", loadingArticles: "Завантаження статей…", noArticles: "Статей не знайдено.",
    nextPage: "Наступна сторінка", upvote: "Підтримати", downvote: "Не підтримати", voteSaveError: "Не вдалося зберегти голос.",
  },
  pl: {
    newsTab: "Wiadomości", imagesTab: "Obrazy", sortNew: "Nowe", sortVotes: "Głosy",
    sortViews: "Wyświetlenia", periodWeek: "Tydzień", periodMonth: "Miesiąc", periodAll: "Wszystko",
    searchPlaceholder: "Szukaj", loadingArticles: "Ładowanie artykułów…", noArticles: "Nie znaleziono artykułów.",
    nextPage: "Następna strona", upvote: "Głos za", downvote: "Głos przeciw", voteSaveError: "Nie udało się zapisać głosu.",
  },
  ja: {
    newsTab: "ニュース", imagesTab: "画像", sortNew: "新着", sortVotes: "投票",
    sortViews: "閲覧数", periodWeek: "週間", periodMonth: "月間", periodAll: "すべて",
    searchPlaceholder: "検索", loadingArticles: "記事を読み込み中…", noArticles: "記事が見つかりません。",
    nextPage: "次のページ", upvote: "高評価", downvote: "低評価", voteSaveError: "投票を保存できませんでした。",
  },
  zh: {
    newsTab: "新闻", imagesTab: "图片", sortNew: "最新", sortVotes: "投票",
    sortViews: "浏览量", periodWeek: "本周", periodMonth: "本月", periodAll: "全部",
    searchPlaceholder: "搜索", loadingArticles: "正在加载文章…", noArticles: "未找到文章。",
    nextPage: "下一页", upvote: "赞成", downvote: "反对", voteSaveError: "无法保存投票。",
  },
  ar: {
    newsTab: "الأخبار", imagesTab: "الصور", sortNew: "الأحدث", sortVotes: "الأصوات",
    sortViews: "المشاهدات", periodWeek: "أسبوع", periodMonth: "شهر", periodAll: "الكل",
    searchPlaceholder: "بحث", loadingArticles: "جارٍ تحميل المقالات…", noArticles: "لم يتم العثور على مقالات.",
    nextPage: "الصفحة التالية", upvote: "تصويت مؤيد", downvote: "تصويت معارض", voteSaveError: "تعذر حفظ التصويت.",
  },
  hi: {
    newsTab: "समाचार", imagesTab: "चित्र", sortNew: "नए", sortVotes: "वोट",
    sortViews: "दृश्य", periodWeek: "सप्ताह", periodMonth: "महीना", periodAll: "सभी",
    searchPlaceholder: "खोजें", loadingArticles: "लेख लोड हो रहे हैं…", noArticles: "कोई लेख नहीं मिला।",
    nextPage: "अगला पृष्ठ", upvote: "समर्थन में वोट", downvote: "विरोध में वोट", voteSaveError: "वोट सहेजा नहीं जा सका।",
  },
};

const ENGLISH_EXTRA_COPY: ExtraCopy = {
  imagesLoadError: "Images could not be loaded.",
  loadingImages: "Loading images…",
  noImages: "No images found.",
  signInToTranslate: "Sign in to generate this translation.",
  loadingTranslation: "Loading translation…",
  translatingArticle: "Translating article…",
  translationGenerationError: "Translation could not be generated.",
  translationFailed: "Translation failed.",
  contentLoadError: "Failed to load content.",
};

const EXTRA_COPY_BY_LANGUAGE: Record<string, ExtraCopy> = {
  en: ENGLISH_EXTRA_COPY,
  pt: {
    imagesLoadError: "Não foi possível carregar as imagens.",
    loadingImages: "Carregando imagens…",
    noImages: "Nenhuma imagem encontrada.",
    signInToTranslate: "Entre para gerar esta tradução.",
    loadingTranslation: "Carregando tradução…",
    translatingArticle: "Traduzindo artigo…",
    translationGenerationError: "Não foi possível gerar a tradução.",
    translationFailed: "A tradução falhou.",
    contentLoadError: "Não foi possível carregar o conteúdo.",
  },
  es: {
    imagesLoadError: "No se pudieron cargar las imágenes.", loadingImages: "Cargando imágenes…", noImages: "No se encontraron imágenes.",
    signInToTranslate: "Inicia sesión para generar esta traducción.", loadingTranslation: "Cargando traducción…", translatingArticle: "Traduciendo artículo…",
    translationGenerationError: "No se pudo generar la traducción.", translationFailed: "La traducción falló.", contentLoadError: "No se pudo cargar el contenido.",
  },
  fr: {
    imagesLoadError: "Impossible de charger les images.", loadingImages: "Chargement des images…", noImages: "Aucune image trouvée.",
    signInToTranslate: "Connectez-vous pour générer cette traduction.", loadingTranslation: "Chargement de la traduction…", translatingArticle: "Traduction de l’article…",
    translationGenerationError: "Impossible de générer la traduction.", translationFailed: "La traduction a échoué.", contentLoadError: "Impossible de charger le contenu.",
  },
  de: {
    imagesLoadError: "Bilder konnten nicht geladen werden.", loadingImages: "Bilder werden geladen…", noImages: "Keine Bilder gefunden.",
    signInToTranslate: "Melde dich an, um diese Übersetzung zu erstellen.", loadingTranslation: "Übersetzung wird geladen…", translatingArticle: "Artikel wird übersetzt…",
    translationGenerationError: "Die Übersetzung konnte nicht erstellt werden.", translationFailed: "Die Übersetzung ist fehlgeschlagen.", contentLoadError: "Der Inhalt konnte nicht geladen werden.",
  },
  it: {
    imagesLoadError: "Impossibile caricare le immagini.", loadingImages: "Caricamento immagini…", noImages: "Nessuna immagine trovata.",
    signInToTranslate: "Accedi per generare questa traduzione.", loadingTranslation: "Caricamento traduzione…", translatingArticle: "Traduzione dell’articolo…",
    translationGenerationError: "Impossibile generare la traduzione.", translationFailed: "La traduzione non è riuscita.", contentLoadError: "Impossibile caricare il contenuto.",
  },
  uk: {
    imagesLoadError: "Не вдалося завантажити зображення.", loadingImages: "Завантаження зображень…", noImages: "Зображень не знайдено.",
    signInToTranslate: "Увійдіть, щоб створити цей переклад.", loadingTranslation: "Завантаження перекладу…", translatingArticle: "Переклад статті…",
    translationGenerationError: "Не вдалося створити переклад.", translationFailed: "Помилка перекладу.", contentLoadError: "Не вдалося завантажити вміст.",
  },
  pl: {
    imagesLoadError: "Nie udało się załadować obrazów.", loadingImages: "Ładowanie obrazów…", noImages: "Nie znaleziono obrazów.",
    signInToTranslate: "Zaloguj się, aby wygenerować to tłumaczenie.", loadingTranslation: "Ładowanie tłumaczenia…", translatingArticle: "Tłumaczenie artykułu…",
    translationGenerationError: "Nie udało się wygenerować tłumaczenia.", translationFailed: "Tłumaczenie nie powiodło się.", contentLoadError: "Nie udało się załadować treści.",
  },
  ja: {
    imagesLoadError: "画像を読み込めませんでした。", loadingImages: "画像を読み込み中…", noImages: "画像が見つかりません。",
    signInToTranslate: "この翻訳を生成するにはログインしてください。", loadingTranslation: "翻訳を読み込み中…", translatingArticle: "記事を翻訳中…",
    translationGenerationError: "翻訳を生成できませんでした。", translationFailed: "翻訳に失敗しました。", contentLoadError: "コンテンツを読み込めませんでした。",
  },
  zh: {
    imagesLoadError: "无法加载图片。", loadingImages: "正在加载图片…", noImages: "未找到图片。",
    signInToTranslate: "请登录以生成此翻译。", loadingTranslation: "正在加载翻译…", translatingArticle: "正在翻译文章…",
    translationGenerationError: "无法生成翻译。", translationFailed: "翻译失败。", contentLoadError: "无法加载内容。",
  },
  ar: {
    imagesLoadError: "تعذر تحميل الصور.", loadingImages: "جارٍ تحميل الصور…", noImages: "لم يتم العثور على صور.",
    signInToTranslate: "سجّل الدخول لإنشاء هذه الترجمة.", loadingTranslation: "جارٍ تحميل الترجمة…", translatingArticle: "جارٍ ترجمة المقال…",
    translationGenerationError: "تعذر إنشاء الترجمة.", translationFailed: "فشلت الترجمة.", contentLoadError: "تعذر تحميل المحتوى.",
  },
  hi: {
    imagesLoadError: "चित्र लोड नहीं किए जा सके।", loadingImages: "चित्र लोड हो रहे हैं…", noImages: "कोई चित्र नहीं मिला।",
    signInToTranslate: "यह अनुवाद बनाने के लिए साइन इन करें।", loadingTranslation: "अनुवाद लोड हो रहा है…", translatingArticle: "लेख का अनुवाद हो रहा है…",
    translationGenerationError: "अनुवाद बनाया नहीं जा सका।", translationFailed: "अनुवाद विफल रहा।", contentLoadError: "सामग्री लोड नहीं की जा सकी।",
  },
};
export const globalCopyForLanguage = (language: string | null): GlobalCopy => {
  if (!language) return { ...ENGLISH_COPY, ...ENGLISH_HOME_COPY, ...ENGLISH_EXTRA_COPY };
  try {
    const locale = new Intl.Locale(language);
    return {
      ...(COPY_BY_LANGUAGE[locale.language] ?? ENGLISH_COPY),
      ...(HOME_COPY_BY_LANGUAGE[locale.language] ?? ENGLISH_HOME_COPY),
      ...(EXTRA_COPY_BY_LANGUAGE[locale.language] ?? ENGLISH_EXTRA_COPY),
    };
  } catch {
    return { ...ENGLISH_COPY, ...ENGLISH_HOME_COPY, ...ENGLISH_EXTRA_COPY };
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
  copy: globalCopyForLanguage(null),
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
