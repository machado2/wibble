import { GlobalOutlined } from "@ant-design/icons";
import { useEffect, useMemo, useRef, useState } from "react";
import { orderedTranslationLanguages } from "@/core/translationLanguages";
import { useGlobalLanguage } from "./GlobalLanguage";
import styles from "./GlobalLanguageSelector.module.css";

export const GlobalLanguageSelector = () => {
  const { language, copy, setLanguage } = useGlobalLanguage();
  const [open, setOpen] = useState(false);
  const wrapper = useRef<HTMLDivElement>(null);
  const browserLanguages = useMemo(() =>
    typeof navigator === "undefined"
      ? ["en"]
      : navigator.languages?.length
        ? Array.from(navigator.languages)
        : [navigator.language || "en"], []);
  const choices = useMemo(
    () => orderedTranslationLanguages(browserLanguages, language ?? browserLanguages[0] ?? "en"),
    [browserLanguages, language]
  );

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!wrapper.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);

  return (
    <div className={styles.wrapper} ref={wrapper}>
      <button
        type="button"
        className={styles.trigger}
        aria-label={copy.chooseLanguage}
        title={copy.chooseLanguage}
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <GlobalOutlined aria-hidden="true" />
        <span>{language ? language.split("-")[0].toUpperCase() : "EN"}</span>
      </button>
      {open ? (
        <div className={styles.menu} role="menu" aria-label={copy.chooseLanguage}>
          {choices.map((choice) => (
            <button
              type="button"
              role="menuitemradio"
              aria-checked={language === choice.code}
              key={choice.code}
              onClick={() => { setOpen(false); void setLanguage(choice.code); }}
            >
              <span>{choice.label}</span>
              <small>{choice.code}</small>
            </button>
          ))}
          <button
            type="button"
            role="menuitemradio"
            aria-checked={language === null}
            onClick={() => { setOpen(false); void setLanguage(null); }}
          >
            <span>{copy.originalLanguage}</span>
            <small>EN</small>
          </button>
        </div>
      ) : null}
    </div>
  );
};
