import React from "react";
import {
  Admin,
  I18nProvider,
  Resource,
  defaultI18nProvider,
} from "react-admin";
import { dataProvider } from "ra-data-simple-prisma";
import {
  FaExclamationTriangle,
  FaImages,
  FaLanguage,
  FaNewspaper,
  FaRoute,
  FaSearch,
} from "react-icons/fa";
import { authProvider } from "./authProvider";
import { ContentList, ContentEdit, ContentCreate } from "./Content";
import { ImageCacheList, ImageCacheEdit } from "./ImageCache";
import { NotFoundRequestList } from "./NotFoundRequests";
import { SearchHistoryList, GenerationFailureList } from "./Activity";
import { Dashboard } from "./Dashboard";
import { AdminLayout } from "./AdminLayout";
import { TranslationJobList } from "./TranslationJobs";

const prismaDataProvider = dataProvider("/api/adm");

const adminMessages: Record<
  string,
  string | ((options: Record<string, unknown>) => string)
> = {
  "ra.action.add_filter": "Adicionar filtro",
  "ra.action.cancel": "Cancelar",
  "ra.action.confirm": "Confirmar",
  "ra.action.create": "Criar",
  "ra.action.delete": "Excluir",
  "ra.action.export": "Exportar",
  "ra.action.refresh": "Atualizar",
  "ra.action.save": "Salvar",
  "ra.action.search": "Buscar",
  "ra.auth.user_menu": "Perfil",
  "ra.navigation.no_results": "Nenhum resultado encontrado",
  "ra.navigation.page_rows_per_page": "Linhas por página:",
  "ra.navigation.page_range_info": (options) =>
    `${options.offsetBegin}-${options.offsetEnd} de ${options.total}`,
  "ra.navigation.next": "Próxima",
  "ra.navigation.prev": "Anterior",
};

const adminI18nProvider: I18nProvider = {
  translate: (key, options = {}) => {
    const message = adminMessages[key];
    if (typeof message === "function") return message(options);
    return message ?? defaultI18nProvider.translate(key, options);
  },
  changeLocale: () => Promise.resolve(),
  getLocale: () => "pt-BR",
};

const adminTheme = {
  palette: {
    mode: "light" as const,
    primary: { main: "#345fa8", contrastText: "#ffffff" },
    secondary: { main: "#ff705e", contrastText: "#ffffff" },
    error: { main: "#c53b29" },
    warning: { main: "#c67a10" },
    success: { main: "#187a41" },
    background: { default: "#f5f6f2", paper: "#ffffff" },
    text: { primary: "#1d2939", secondary: "#667085" },
  },
  shape: { borderRadius: 12 },
  typography: {
    fontFamily:
      'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    h6: { fontWeight: 800, letterSpacing: "-0.02em" },
    button: { fontWeight: 800, textTransform: "none" as const },
  },
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        body: { backgroundColor: "#f5f6f2" },
      },
    },
    RaLayout: {
      styleOverrides: {
        root: {
          "& .RaLayout-content": {
            maxWidth: "1600px",
            padding: "28px 30px 46px",
          },
        },
      },
    },
    RaSidebar: {
      styleOverrides: {
        root: {
          backgroundColor: "#101a2e",
          borderRight: 0,
          "& .MuiPaper-root": { backgroundColor: "#101a2e", borderRight: 0 },
        },
      },
    },
    RaMenuItemLink: {
      styleOverrides: {
        root: {
          minHeight: 44,
          marginBottom: 4,
          borderRadius: 10,
          color: "#b8c3d6",
          "& .MuiListItemIcon-root": { color: "#7f8ba2" },
          "&:hover": { backgroundColor: "rgba(255,255,255,.07)", color: "#fff" },
          "&.RaMenuItemLink-active": {
            backgroundColor: "#d9ff57",
            color: "#101a2e",
            "& .MuiListItemIcon-root": { color: "#101a2e" },
          },
        },
      },
    },
    MuiPaper: {
      styleOverrides: {
        root: {
          backgroundImage: "none",
        },
        elevation1: {
          border: "1px solid #e4e7ec",
          boxShadow: "0 8px 28px rgba(16, 24, 40, 0.045)",
        },
      },
    },
    MuiButton: {
      styleOverrides: {
        root: { borderRadius: 10, minHeight: 38 },
        contained: { boxShadow: "none" },
      },
    },
    MuiOutlinedInput: {
      styleOverrides: {
        root: { backgroundColor: "#fff", borderRadius: 10 },
      },
    },
    MuiTableHead: {
      styleOverrides: {
        root: {
          backgroundColor: "#f8fafc",
          "& .MuiTableCell-root": {
            color: "#667085",
            fontSize: 10,
            fontWeight: 800,
            letterSpacing: ".06em",
            textTransform: "uppercase",
          },
        },
      },
    },
    MuiTableCell: {
      styleOverrides: {
        root: { borderColor: "#edf0f4", paddingTop: 12, paddingBottom: 12 },
      },
    },
    MuiTableRow: {
      styleOverrides: {
        root: {
          transition: "background-color 120ms ease",
          "&:hover": { backgroundColor: "#f9faf5 !important" },
        },
      },
    },
    RaToolbar: {
      styleOverrides: {
        root: { borderRadius: "0 0 12px 12px" },
      },
    },
  },
};

export const AdminPanel: React.FC = () => (
  <Admin
    dataProvider={prismaDataProvider}
    authProvider={authProvider}
    i18nProvider={adminI18nProvider}
    dashboard={Dashboard}
    layout={AdminLayout}
    theme={adminTheme}
    title="Wibble Control"
    disableTelemetry
  >
    <Resource
      name="content"
      list={ContentList}
      edit={ContentEdit}
      create={ContentCreate}
      icon={FaNewspaper}
      options={{ label: "Artigos" }}
    />
    <Resource
      name="image_cache"
      list={ImageCacheList}
      edit={ImageCacheEdit}
      icon={FaImages}
      options={{ label: "Imagens" }}
    />
    <Resource
      name="translation_job"
      list={TranslationJobList}
      icon={FaLanguage}
      options={{ label: "Traduções" }}
    />
    <Resource
      name="search_history"
      list={SearchHistoryList}
      icon={FaSearch}
      options={{ label: "Buscas" }}
    />
    <Resource
      name="history_generation_fail"
      list={GenerationFailureList}
      icon={FaExclamationTriangle}
      options={{ label: "Falhas" }}
    />
    <Resource
      name="not_found_request"
      list={NotFoundRequestList}
      icon={FaRoute}
      options={{ label: "Rotas 404" }}
    />
  </Admin>
);
