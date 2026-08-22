import React, { useCallback, useEffect, useRef, useState } from "react";
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
  translations: {
    pending: number;
    processing: number;
    completed: number;
    failed: number;
    total: number;
  };
  recentTranslationJobs: Array<{
    id: string;
    slug: string;
    title: string;
    language_code: string;
    status: "pending" | "processing" | "completed" | "failed";
    attempts: number;
    created_at: string;
    updated_at: string;
    next_attempt_at: string;
    started_at: string | null;
    completed_at: string | null;
    last_error: string | null;
  }>;
  translationOperations: {
    updatedAt: string;
    cost: {
      limit: number;
      used: number;
      remaining: number;
      windowMinutes: number;
      nextSlotAt: string | null;
      exhausted: boolean;
    };
    queue: {
      pending: number;
      processing: number;
      completed: number;
      failed: number;
      total: number;
      settled: number;
      progressPercent: number;
      waitingForQuota: number;
      oldestPendingAt: string | null;
      nextAttemptAt: string | null;
    };
    throughput: {
      completedLastHour: number;
      completedLast24Hours: number;
      averageDurationSeconds: number | null;
    };
    coverage: {
      eligibleArticles: number;
      activeLanguages: number;
      translated: number;
      possible: number;
      percent: number;
      languages: Array<{
        languageCode: string;
        translated: number;
        pending: number;
        processing: number;
        completed: number;
        failed: number;
        waitingForQuota: number;
        percent: number;
      }>;
    };
  };
};

const translationStatus = (status: string) => {
  if (status === "completed") return <StatusBadge label="Concluída" tone="success" />;
  if (status === "processing") return <StatusBadge label="Processando" tone="warning" />;
  if (status === "failed") return <StatusBadge label="Falhou" tone="danger" />;
  return <StatusBadge label="Pendente" tone="neutral" />;
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

const ProgressBar = ({ value, tone = "default", label }: {
  value: number;
  tone?: "default" | "warning" | "success";
  label: string;
}) => (
  <div
    className={`${styles.progressTrack} ${styles[`progressTrack${tone[0].toUpperCase()}${tone.slice(1)}`]}`}
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={value}
  >
    <span style={{ width: `${Math.max(0, Math.min(100, value))}%` }} />
  </div>
);

const formatDuration = (seconds: number | null) => {
  if (seconds === null) return "—";
  if (seconds < 60) return `${Math.round(seconds)} s`;
  return `${Math.round(seconds / 60)} min`;
};

export const Dashboard = () => {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const overviewRequest = useRef<Promise<void> | null>(null);

  const loadOverview = useCallback(() => {
    if (overviewRequest.current) return overviewRequest.current;
    const request = (async () => {
      setError(null);
      try {
        const response = await fetch("/api/admin/overview");
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        setOverview((await response.json()) as Overview);
      } catch (loadError) {
        console.error("Failed to load admin overview", loadError);
        setError("Não foi possível atualizar os indicadores agora.");
      }
    })();
    overviewRequest.current = request;
    void request.finally(() => {
      if (overviewRequest.current === request) overviewRequest.current = null;
    });
    return request;
  }, []);

  useEffect(() => {
    let cancelled = false;
    let refresh: number | undefined;
    const scheduleRefresh = () => {
      refresh = window.setTimeout(async () => {
        if (!document.hidden) await loadOverview();
        if (!cancelled) scheduleRefresh();
      }, 15_000);
    };
    void loadOverview().finally(() => {
      if (!cancelled) scheduleRefresh();
    });
    return () => {
      cancelled = true;
      if (refresh !== undefined) window.clearTimeout(refresh);
    };
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

      {overview && error ? (
        <div className={styles.refreshWarning} role="status">
          <span>{error} Os últimos dados válidos continuam visíveis.</span>
          <button onClick={() => void loadOverview()}>Tentar novamente</button>
        </div>
      ) : null}

      {!overview && !error ? (
        <div className={`${styles.panel} ${styles.loadingState}`}>
          <div>
            <FaSpinner />
            <p>Reunindo os sinais da operação…</p>
          </div>
        </div>
      ) : !overview && error ? (
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
          <section className={`${styles.panel} ${styles.translationPanel}`}>
            <div className={styles.panelHeader}>
              <div>
                <h2>Traduções em background</h2>
                <span className={styles.liveUpdate}>
                  Atualização automática · {formatDateTime(overview.translationOperations.updatedAt)}
                </span>
              </div>
              <a className={styles.panelLink} href="#/translation_job">
                Ver todos os jobs
              </a>
            </div>

            <div className={styles.operationsGrid}>
              <article className={styles.operationCard}>
                <span className={styles.operationLabel}>Orçamento automático</span>
                <strong>
                  {overview.translationOperations.cost.used} de {overview.translationOperations.cost.limit} usadas
                </strong>
                <ProgressBar
                  value={(overview.translationOperations.cost.used / overview.translationOperations.cost.limit) * 100}
                  tone={overview.translationOperations.cost.exhausted ? "warning" : "success"}
                  label="Consumo da quota automática"
                />
                <span className={styles.operationHighlight}>
                  {overview.translationOperations.cost.remaining} disponíveis agora
                </span>
                <p>
                  Janela móvel de {overview.translationOperations.cost.windowMinutes} min.
                  {overview.translationOperations.cost.exhausted && overview.translationOperations.cost.nextSlotAt
                    ? ` Próxima vaga às ${formatDateTime(overview.translationOperations.cost.nextSlotAt)}.`
                    : " Há capacidade disponível agora."}
                </p>
              </article>

              <article className={styles.operationCard}>
                <span className={styles.operationLabel}>Progresso da fila</span>
                <strong>{overview.translationOperations.queue.progressPercent}% dos jobs encerrados</strong>
                <ProgressBar
                  value={overview.translationOperations.queue.progressPercent}
                  label="Progresso da fila de traduções"
                />
                <span className={styles.operationHighlight}>
                  {overview.translationOperations.queue.waitingForQuota} esperando a quota
                </span>
                <p>
                  {overview.translationOperations.queue.pending} pendentes · {overview.translationOperations.queue.processing} processando · {overview.translationOperations.queue.failed} falharam
                </p>
              </article>

              <article className={styles.operationCard}>
                <span className={styles.operationLabel}>Cobertura persistida</span>
                <strong>
                  {overview.translationOperations.coverage.translated} de {overview.translationOperations.coverage.possible}
                </strong>
                <ProgressBar
                  value={overview.translationOperations.coverage.percent}
                  tone="success"
                  label="Cobertura persistida das traduções"
                />
                <span className={styles.operationHighlight}>
                  {overview.translationOperations.coverage.percent}% da cobertura solicitada
                </span>
                <p>
                  {overview.translationOperations.coverage.eligibleArticles} artigos elegíveis × {overview.translationOperations.coverage.activeLanguages} idiomas já solicitados.
                </p>
              </article>
            </div>

            <div className={styles.costExplanation}>
              <strong>Como o custo é controlado</strong>
              <p>
                O limite é global para toda a automação: no máximo {overview.translationOperations.cost.limit} gerações pagas por hora. Uma geração produz título, descrição e conteúdo; cache hits e espera por quota não consomem uma vaga.
              </p>
            </div>

            <div className={styles.translationDetailsGrid}>
              <div className={styles.coverageSection}>
                <div className={styles.subsectionHeader}>
                  <h3>Cobertura por idioma</h3>
                  <span>{overview.translationOperations.coverage.activeLanguages} ativos</span>
                </div>
                <div className={styles.languageList}>
                  {overview.translationOperations.coverage.languages.map((language) => (
                    <div className={styles.languageRow} key={language.languageCode}>
                      <div className={styles.languageHeading}>
                        <strong>{language.languageCode.toUpperCase()}</strong>
                        <span>{language.translated}/{overview.translationOperations.coverage.eligibleArticles} traduzidos</span>
                        <b>{language.percent}%</b>
                      </div>
                      <ProgressBar
                        value={language.percent}
                        tone="success"
                        label={`Cobertura de ${language.languageCode}`}
                      />
                      <div className={styles.languageMeta}>
                        <span>{language.pending} pendentes</span>
                        <span>{language.processing} processando</span>
                        <span>{language.failed} falhas</span>
                        {language.waitingForQuota ? <span>{language.waitingForQuota} na quota</span> : null}
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <div className={styles.queueFacts}>
                <div className={styles.subsectionHeader}>
                  <h3>Ritmo e espera</h3>
                </div>
                <dl>
                  <div><dt>Concluídas na última hora</dt><dd>{overview.translationOperations.throughput.completedLastHour}</dd></div>
                  <div><dt>Concluídas em 24h</dt><dd>{overview.translationOperations.throughput.completedLast24Hours}</dd></div>
                  <div><dt>Tempo médio por geração</dt><dd>{formatDuration(overview.translationOperations.throughput.averageDurationSeconds)}</dd></div>
                  <div><dt>Job pendente mais antigo</dt><dd>{overview.translationOperations.queue.oldestPendingAt ? formatDateTime(overview.translationOperations.queue.oldestPendingAt) : "—"}</dd></div>
                  <div><dt>Próxima tentativa prevista</dt><dd>{overview.translationOperations.queue.nextAttemptAt ? formatDateTime(overview.translationOperations.queue.nextAttemptAt) : "—"}</dd></div>
                </dl>
              </div>
            </div>

            <div className={styles.subsectionHeaderWithPadding}>
              <h3>Atividade recente</h3>
              <div className={styles.translationSummary}>
                <StatusBadge label={`${overview.translations.pending} pendentes`} tone="neutral" />
                <StatusBadge label={`${overview.translations.processing} em processamento`} tone="warning" />
                <StatusBadge label={`${overview.translations.completed} concluídas`} tone="success" />
                <StatusBadge label={`${overview.translations.failed} falhas`} tone="danger" />
              </div>
            </div>
            {overview.recentTranslationJobs.length ? (
              <ul className={styles.recentList}>
                {overview.recentTranslationJobs.map((job) => (
                  <li className={styles.recentItem} key={job.id}>
                    <div>
                      <span className={styles.recentTitle}>{job.title}</span>
                      <div className={styles.recentMeta}>
                        {translationStatus(job.status)}
                        <span>{job.language_code}</span>
                        <span>·</span>
                        <span>{job.attempts} tentativa(s)</span>
                        <span>·</span>
                        <span>{formatDateTime(job.updated_at)}</span>
                      </div>
                      {job.last_error ? (
                        <span className={styles.translationError}>{job.last_error}</span>
                      ) : null}
                    </div>
                    <a
                      className={styles.iconLink}
                      href={`/content/${job.slug}?lang=${encodeURIComponent(job.language_code)}`}
                      target="_blank"
                      rel="noreferrer"
                      title="Abrir tradução"
                    >
                      <FaExternalLinkAlt />
                    </a>
                  </li>
                ))}
              </ul>
            ) : (
              <div className={styles.emptyState}>Nenhum job de tradução criado ainda.</div>
            )}
          </section>

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
