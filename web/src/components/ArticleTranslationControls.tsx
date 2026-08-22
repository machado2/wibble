import { CheckOutlined, GlobalOutlined, LoadingOutlined } from "@ant-design/icons";
import { useRouter } from "next/router";
import { useEffect, useMemo, useState } from "react";
import {
  languageLabel,
  normalizeLanguageCode,
  orderedTranslationLanguages,
  type TranslationLanguage,
} from "@/core/translationLanguages";
import styles from "./ArticleTranslationControls.module.css";

type Props = {
  slug: string;
  currentLanguage: string | null;
  availableLanguages: string[];
};

export const ArticleTranslationControls = ({
  slug,
  currentLanguage,
  availableLanguages,
}: Props) => {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [browserLanguages, setBrowserLanguages] = useState<string[]>(["en"]);
  const [translating, setTranslating] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const requested = navigator.languages?.length
      ? Array.from(navigator.languages)
      : [navigator.language || "en"];
    setBrowserLanguages(requested);
  }, []);

  const choices = useMemo(() => {
    const displayLocale = browserLanguages[0] ?? "en";
    const preferred = orderedTranslationLanguages(
      browserLanguages,
      displayLocale
    );
    const known = new Set(preferred.map((language) => language.code));
    const cached = availableLanguages.flatMap<TranslationLanguage>((value) => {
      try {
        const code = normalizeLanguageCode(value);
        if (known.has(code)) return [];
        known.add(code);
        return [
          {
            code,
            label: languageLabel(code, displayLocale),
            preferred: false,
          },
        ];
      } catch {
        return [];
      }
    });
    return [...preferred, ...cached];
  }, [availableLanguages, browserLanguages]);

  const navigateTo = async (language: string | null) => {
    setOpen(false);
    setError(null);
    await router.push({
      pathname: "/content/[slug]",
      query: language ? { slug, lang: language } : { slug },
    });
  };

  const selectLanguage = async (language: string) => {
    if (language === currentLanguage) {
      setOpen(false);
      return;
    }
    if (availableLanguages.includes(language)) {
      await navigateTo(language);
      return;
    }

    setTranslating(language);
    setError(null);
    try {
      const response = await fetch(`/api/translations/${encodeURIComponent(slug)}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ language }),
      });
      const payload = await response.json().catch(() => null);
      if (!response.ok) {
        throw new Error(payload?.error || "Translation could not be generated");
      }
      await navigateTo(payload.languageCode || language);
    } catch (translationError) {
      setError(
        translationError instanceof Error
          ? translationError.message
          : "Translation could not be generated"
      );
    } finally {
      setTranslating(null);
    }
  };

  const currentLabel = currentLanguage
    ? languageLabel(currentLanguage, browserLanguages[0] ?? "en")
    : "Original";

  return (
    <section className={styles.wrapper} aria-label="Article language">
      <button
        type="button"
        className={styles.trigger}
        aria-expanded={open}
        aria-controls="article-language-options"
        aria-label="Translate or change language"
        onClick={() => {
          setOpen((value) => !value);
          setError(null);
        }}
      >
        <GlobalOutlined aria-hidden="true" />
        <span>Translate</span>
        <span className={styles.current}>{currentLabel}</span>
      </button>

      {open ? (
        <div className={styles.panel} id="article-language-options">
          <div className={styles.panelHeader}>
            <strong>Read in another language</strong>
            <span>Translations stay attached to this article.</span>
          </div>
          <div className={styles.choiceList}>
            {choices.map((language) => {
              const available = availableLanguages.includes(language.code);
              const active = currentLanguage === language.code;
              const busy = translating === language.code;
              return (
                <button
                  key={language.code}
                  type="button"
                  data-testid="language-choice"
                  className={`${styles.choice} ${active ? styles.active : ""}`}
                  disabled={translating !== null}
                  onClick={() => void selectLanguage(language.code)}
                >
                  <span>
                    <strong>{language.label}</strong>
                    <small>
                      {language.preferred
                        ? "Browser language"
                        : available
                          ? "Available now"
                          : "Generate translation"}
                    </small>
                  </span>
                  {busy ? (
                    <LoadingOutlined spin aria-label="Translating" />
                  ) : active || available ? (
                    <CheckOutlined aria-hidden="true" />
                  ) : (
                    <span className={styles.generate}>Translate</span>
                  )}
                </button>
              );
            })}
            <button
              type="button"
              className={`${styles.choice} ${currentLanguage === null ? styles.active : ""}`}
              disabled={translating !== null}
              onClick={() => void navigateTo(null)}
            >
              <span>
                <strong>Original</strong>
                <small>Article as first published</small>
              </span>
              {currentLanguage === null ? <CheckOutlined aria-hidden="true" /> : null}
            </button>
          </div>
          {error ? <p className={styles.error}>{error}</p> : null}
        </div>
      ) : null}
    </section>
  );
};
