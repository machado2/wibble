import React, { useCallback, useEffect, useState } from "react";
import {
  FaCheck,
  FaExclamationTriangle,
  FaExternalLinkAlt,
  FaEye,
  FaNewspaper,
  FaPen,
  FaRoute,
  FaSearch,
  FaSpinner,
} from "react-icons/fa";
import {
  PageHeader,
  StatusBadge,
  formatDateTime,
  formatNumber,
} from "./AdminUI";
import styles from "./Admin.module.css";

type RecentArticle = {
  id: string;
  slug: string;
  title: string;
  created_at: string;
  generating: boolean;
  flagged: boolean;
  published: boolean;
  votes: number;
  view_count: number;
};

type Overview = {
  totalArticles: number;
  publishedArticles: number;
  generatingArticles: number;
  flaggedArticles: number;
  unresolvedUrls: number;
  searchesLast24Hours: number;
  images: Record<string, number>;
  recentArticles: RecentArticle[];
};

const articleStatus = (article: RecentArticle) => {
  if (article.flagged) return <StatusBadge label="Sinalizado" tone="danger" />;
  if (article.generating)
    return <StatusBadge label="Gerando" tone="warning" />;
  if (article.published)
    return <StatusBadge label="Publicado" tone="success" />;
  return <StatusBadge label="Rascunho" tone="neutral" />;
};

const MetricCard = ({
  icon,
  value,
  label,
  accent,
}: {
  icon: React.ReactNode;
  value: number;
  label: string;
  accent: string;
}) => (
  <div
    className={styles.metricCard}
    style={{ "--metric-accent": accent } as React.CSSProperties}
  >
    <span className={styles.metricIcon}>{icon}</span>
    <strong className={styles.metricValue}>{formatNumber(value)}</strong>
    <span className={styles.metricLabel}>{label}</span>
  </div>
);

export const Dashboard = () => {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadOverview = useCallback(async () => {
    setError(null);
    try {
      const response = await fetch("/api/admin/overview");
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      setOverview((await response.json()) as Overview);
    } catch (loadError) {
      console.error("Failed to load admin overview", loadError);
      setError("Não foi possível carregar os indicadores agora.");
    }
  }, []);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  const pendingImages = overview
    ? Object.entries(overview.images).reduce(
        (total, [status, count]) =>
          status === "completed" ? total : total + count,
        0
      )
    : 0;

  return (
    <div className={styles.page}>
      <PageHeader
        eyebrow="Visão geral"
        title="Sala de controle"
        description="O pulso editorial do Wibble: o que está no ar, o que exige atenção e o que acabou de chegar."
      />

      {!overview && !error ? (
        <div className={`${styles.panel} ${styles.loadingState}`}>
          <div>
            <FaSpinner />
            <p>Reunindo os sinais da operação…</p>
          </div>
        </div>
      ) : error ? (
        <div className={`${styles.panel} ${styles.errorState}`}>
          <div>
            <FaExclamationTriangle />
            <p>{error}</p>
            <button className={styles.retryButton} onClick={() => void loadOverview()}>
              Tentar novamente
            </button>
          </div>
        </div>
      ) : overview ? (
        <>
          <div className={styles.metricGrid}>
            <MetricCard
              icon={<FaNewspaper />}
              value={overview.publishedArticles}
              label="artigos publicados"
              accent="#d9ff57"
            />
            <MetricCard
              icon={<FaSpinner />}
              value={overview.generatingArticles}
              label="artigos em geração"
              accent="#ffd166"
            />
            <MetricCard
              icon={<FaRoute />}
              value={overview.unresolvedUrls}
              label="rotas 404 aguardando"
              accent="#ff705e"
            />
            <MetricCard
              icon={<FaSearch />}
              value={overview.searchesLast24Hours}
              label="buscas nas últimas 24h"
              accent="#8db3ff"
            />
          </div>

          <div className={styles.dashboardGrid}>
            <section className={styles.panel}>
              <div className={styles.panelHeader}>
                <h2>Artigos recentes</h2>
                <a className={styles.panelLink} href="#/content">
                  Ver todos
                </a>
              </div>
              {overview.recentArticles.length ? (
                <ul className={styles.recentList}>
                  {overview.recentArticles.map((article) => (
                    <li className={styles.recentItem} key={article.id}>
                      <div>
                        <span className={styles.recentTitle}>{article.title}</span>
                        <div className={styles.recentMeta}>
                          {articleStatus(article)}
                          <span>{formatDateTime(article.created_at)}</span>
                          <span>·</span>
                          <span>{formatNumber(article.view_count)} views</span>
                          <span>·</span>
                          <span>{formatNumber(article.votes)} votos</span>
                        </div>
                      </div>
                      <div className={styles.recentActions}>
                        <a
                          className={styles.iconLink}
                          href={`#/content/${article.id}`}
                          title="Editar artigo"
                        >
                          <FaPen />
                        </a>
                        <a
                          className={styles.iconLink}
                          href={`/${article.slug}`}
                          target="_blank"
                          rel="noreferrer"
                          title="Abrir artigo"
                        >
                          <FaExternalLinkAlt />
                        </a>
                      </div>
                    </li>
                  ))}
                </ul>
              ) : (
                <div className={styles.emptyState}>Nenhum artigo por aqui ainda.</div>
              )}
            </section>

            <div>
              <section className={styles.panel}>
                <div className={styles.panelHeader}>
                  <h2>Saúde da operação</h2>
                  <FaEye />
                </div>
                <ul className={styles.healthList}>
                  <li className={styles.healthItem}>
                    <span>Total de artigos</span>
                    <span className={styles.healthValue}>
                      {formatNumber(overview.totalArticles)}
                    </span>
                  </li>
                  <li className={styles.healthItem}>
                    <span>Artigos sinalizados</span>
                    <span className={styles.healthValue}>
                      {formatNumber(overview.flaggedArticles)}
                    </span>
                  </li>
                  <li className={styles.healthItem}>
                    <span>Imagens fora de “completed”</span>
                    <span className={styles.healthValue}>{formatNumber(pendingImages)}</span>
                  </li>
                  <li className={styles.healthItem}>
                    <span>Imagens concluídas</span>
                    <span className={styles.healthValue}>
                      {formatNumber(overview.images.completed)}
                    </span>
                  </li>
                </ul>
              </section>
              <div className={styles.notice}>
                <strong>
                  <FaCheck /> Operação protegida
                </strong>
                <p>
                  Exclusões de artigos agora removem relações dependentes na mesma
                  transação e só atualizam a tela depois da confirmação do servidor.
                </p>
              </div>
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
};
