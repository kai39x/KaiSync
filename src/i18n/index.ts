import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import ar from "./ar.json";
import en from "./en.json";

export type Lang = "ar" | "en";

i18n.use(initReactI18next).init({
  resources: { ar: { translation: ar }, en: { translation: en } },
  lng: "ar",
  fallbackLng: "ar",
  interpolation: { escapeValue: false },
  returnNull: false,
});

/** Switch language and mirror the whole document (Arabic = RTL, English = LTR). */
export function applyLanguage(lang: Lang) {
  i18n.changeLanguage(lang);
  document.documentElement.lang = lang;
  document.documentElement.dir = lang === "ar" ? "rtl" : "ltr";
}

export default i18n;
