import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  Datagrid,
  DateField,
  FunctionField,
  List,
  NumberField,
  SelectInput,
  TextField,
  TextInput,
  useListContext,
} from "react-admin";
import { PageHeader, StatusBadge, formatDateTime } from "./AdminUI";
import styles from "./Admin.module.css";

type TranslationJobRecord = {
  id: string;
  content_id: string;
  language_code: string;
  status: string;
  attempts: number;
  lease_id?: string | null;
  created_at: string;
  updated_at: string;
  next_attempt_at: string;
  started_at?: string | null;
  completed_at?: string | null;
  last_error?: string | null;
};

type QueueJob = {
  id: string;
  contentId: string;
  slug: string;
  title: string;
  languageCode: string;
  status: "pending" | "processing";
  attempts: number;
  createdAt: string;
  nextAttemptAt: string;
  startedAt: string | null;
  lastError: string | null;
};

type QueueSnapshot = {
  hourlyLimit: number;
  used: number;
  remaining: number;
  windowMinutes: number;
  nextReleaseAt?: string | null;
  jobs: QueueJob[];
};

const operationsUrl = "/api/admin/translation-operations";

const readOperationResponse = async (response: Response) => {
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload?.message || "A operação falhou.");
  return payload;
};

export const TranslationQueueControls = () => {
  const [snapshot, setSnapshot] = useState<QueueSnapshot | null>(null);
  const [hourlyLimit, setHourlyLimit] = useState("10");
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const inFlight = useRef<{ promise: Promise<void>; version: number } | null>(null);
  const requestVersion = useRef(0);
  const quotaDirty = useRef(false);

  const load = useCallback((force = false) => {
    if (!force && inFlight.current) return inFlight.current.promise;
    const version = ++requestVersion.current;
    const request = (async () => {
      const next = (await readOperationResponse(await fetch(operationsUrl))) as QueueSnapshot;
      if (version !== requestVersion.current) return;
      setSnapshot(next);
      if (!quotaDirty.current) setHourlyLimit(String(next.hourlyLimit));
    })().finally(() => {
      if (inFlight.current?.version === version) inFlight.current = null;
    });
    inFlight.current = { promise: request, version };
    return request;
  }, []);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const cycle = async () => {
      try {
        await load();
      } catch (error) {
        if (!cancelled) setMessage(error instanceof Error ? error.message : "Falha ao atualizar a fila.");
      } finally {
        if (!cancelled) timer = setTimeout(cycle, 15_000);
      }
    };
    void cycle();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [load]);

  const remove = async (job: QueueJob) => {
    setBusy(job.id);
    setMessage(null);
    try {
      await readOperationResponse(
        await fetch(`${operationsUrl}?id=${encodeURIComponent(job.id)}`, { method: "DELETE" })
      );
      setSnapshot((current) =>
        current ? { ...current, jobs: current.jobs.filter((item) => item.id !== job.id) } : current
      );
      setMessage("Tarefa removida da fila.");
      await load(true);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Não foi possível remover a tarefa.");
    } finally {
      setBusy(null);
    }
  };

  const saveLimit = async (event: React.FormEvent) => {
    event.preventDefault();
    const value = Number(hourlyLimit);
    setBusy("quota");
    setMessage(null);
    try {
      await readOperationResponse(
        await fetch(operationsUrl, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ hourlyLimit: value }),
        })
      );
      quotaDirty.current = false;
      setMessage(value === 0 ? "Geração automática pausada." : `Quota alterada para ${value} por hora.`);
      await load(true);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Não foi possível alterar a quota.");
    } finally {
      setBusy(null);
    }
  };

  const resetBudget = async () => {
    if (!window.confirm("Liberar agora todo o orçamento da janela móvel? O histórico administrativo será preservado.")) return;
    setBusy("reset");
    setMessage(null);
    try {
      await readOperationResponse(
        await fetch(operationsUrl, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ action: "reset-budget" }),
        })
      );
      setMessage("Consumo da janela móvel resetado.");
      await load(true);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Não foi possível resetar o consumo.");
    } finally {
      setBusy(null);
    }
  };

  if (!snapshot) return <section className={styles.queuePanel}>Carregando fila operacional…</section>;

  return (
    <section className={styles.queuePanel} aria-labelledby="translation-queue-title">
      <div className={styles.queueHeader}>
        <div>
          <span className={styles.sectionEyebrow}>Controle operacional</span>
          <h2 id="translation-queue-title">Fila automática</h2>
          <p>Ordem real de execução. Tarefas pendentes podem ser removidas com um clique.</p>
        </div>
        <div className={styles.queueBudget} aria-label="Consumo atual da quota">
          <strong>{snapshot.used} / {snapshot.hourlyLimit}</strong>
          <span>{snapshot.remaining} restantes em {snapshot.windowMinutes} min</span>
        </div>
      </div>

      <div className={styles.quotaControls}>
        <form onSubmit={saveLimit} className={styles.quotaForm}>
          <label htmlFor="translation-hourly-limit">Gerações por hora</label>
          <input id="translation-hourly-limit" type="number" min="0" max="100" step="1"
            value={hourlyLimit} onChange={(event) => {
              quotaDirty.current = true;
              setHourlyLimit(event.target.value);
            }} />
          <button type="submit" disabled={busy !== null || !/^\d+$/.test(hourlyLimit)}>
            {busy === "quota" ? "Salvando…" : "Salvar quota"}
          </button>
        </form>
        <button type="button" className={styles.resetButton} disabled={busy !== null} onClick={resetBudget}>
          {busy === "reset" ? "Resetando…" : "Resetar consumo"}
        </button>
        <span className={styles.quotaHint}>Use 0 para pausar novas gerações automáticas.</span>
      </div>

      {message ? <div className={styles.operationMessage} role="status">{message}</div> : null}

      <ol className={styles.operationalQueue}>
        {snapshot.jobs.length ? snapshot.jobs.map((job, index) => (
          <li key={job.id} className={styles.queueItem}>
            <span className={styles.queuePosition}>#{index + 1}</span>
            <div className={styles.queueIdentity}>
              <div>
                <StatusBadge label={job.status === "processing" ? "Processando" : "Pendente"}
                  tone={job.status === "processing" ? "warning" : "neutral"} />
                <strong>{job.languageCode.toUpperCase()}</strong>
              </div>
              <strong>{job.title}</strong>
              <span>/{job.slug} · {job.attempts} tentativa{job.attempts === 1 ? "" : "s"}</span>
              {job.lastError ? <span className={styles.queueError}>{job.lastError}</span> : null}
            </div>
            <button type="button" className={styles.removeJobButton}
              disabled={busy !== null || job.status === "processing"}
              aria-label={job.status === "processing" ? "Em execução" : `Remover ${job.title} da fila`}
              onClick={() => void remove(job)}>
              {job.status === "processing" ? "Em execução" : busy === job.id ? "Removendo…" : "Remover"}
            </button>
          </li>
        )) : <li className={styles.emptyState}>A fila está vazia.</li>}
      </ol>
    </section>
  );
};

const filters = [
  <SelectInput
    key="status"
    source="status"
    label="Estado"
    alwaysOn
    choices={[
      { id: "pending", name: "Pendente" },
      { id: "processing", name: "Processando" },
      { id: "completed", name: "Concluída" },
      { id: "failed", name: "Falhou" },
    ]}
  />,
  <SelectInput
    key="last_error"
    source="last_error"
    label="Motivo"
    choices={[
      { id: "Quota de traduções esgotada", name: "Quota esgotada — rejeitada" },
      { id: "Falha temporária ao gerar tradução", name: "Falha temporária" },
      { id: "Falha ao gerar tradução", name: "Falha definitiva" },
    ]}
  />,
  <TextInput key="language_code" source="language_code" label="Idioma" alwaysOn />,
  <TextInput key="content_id" source="content_id" label="ID do artigo" />,
];

const statusBadge = (record: Pick<TranslationJobRecord, "status" | "last_error">) => {
  if (record.status === "completed") return <StatusBadge label="Concluída" tone="success" />;
  if (record.status === "processing") return <StatusBadge label="Processando" tone="warning" />;
  if (record.status === "failed") return <StatusBadge label="Falhou" tone="danger" />;
  return <StatusBadge label="Pendente" tone="neutral" />;
};

const shortLease = (leaseId?: string | null) =>
  leaseId ? `${leaseId.slice(0, 8)}…` : "—";

export const TranslationJobCard = ({ record }: { record: TranslationJobRecord }) => (
  <article className={styles.jobCard}>
    <div className={styles.jobCardHeader}>
      {statusBadge(record)}
      <strong>{record.language_code.toUpperCase()}</strong>
    </div>
    <span className={styles.jobArticleId} title={record.content_id}>
      Artigo {record.content_id}
    </span>
    <dl className={styles.jobFacts}>
      <div><dt>Tentativas</dt><dd>{record.attempts}</dd></div>
      <div><dt>Lease</dt><dd title={record.lease_id || undefined}>{shortLease(record.lease_id)}</dd></div>
      <div><dt>Criada</dt><dd>{formatDateTime(record.created_at)}</dd></div>
      <div><dt>Atualizada</dt><dd>{formatDateTime(record.updated_at)}</dd></div>
      <div><dt>Próxima tentativa</dt><dd>{record.next_attempt_at ? formatDateTime(record.next_attempt_at) : "—"}</dd></div>
      <div><dt>Iniciada</dt><dd>{record.started_at ? formatDateTime(record.started_at) : "—"}</dd></div>
      <div><dt>Concluída</dt><dd>{record.completed_at ? formatDateTime(record.completed_at) : "—"}</dd></div>
    </dl>
    {record.last_error ? (
      <div className={styles.jobError}>
        <span>Último estado/erro</span>
        <strong>{record.last_error}</strong>
      </div>
    ) : null}
  </article>
);

const DesktopJobGrid = () => (
  <Datagrid bulkActionButtons={false}>
    <FunctionField
      label="Estado"
      render={(record: TranslationJobRecord) => statusBadge(record)}
    />
    <TextField source="language_code" label="Idioma" />
    <TextField source="content_id" label="Artigo" />
    <NumberField source="attempts" label="Tentativas" />
    <FunctionField
      label="Lease"
      render={(record: TranslationJobRecord) => (
        <span title={record.lease_id || undefined}>{shortLease(record.lease_id)}</span>
      )}
    />
    <DateField source="created_at" label="Criada" showTime />
    <DateField source="next_attempt_at" label="Próxima tentativa" showTime />
    <DateField source="started_at" label="Iniciada" showTime />
    <DateField source="completed_at" label="Concluída" showTime />
    <FunctionField
      label="Último estado/erro"
      render={(record: TranslationJobRecord) => {
        const value = record.last_error || "—";
        return (
          <span className={styles.cellMeta} title={value}>
            {value}
          </span>
        );
      }}
    />
    <DateField source="updated_at" label="Atualizada" showTime />
  </Datagrid>
);

const ResponsiveJobList = () => {
  const { data, isLoading } = useListContext<TranslationJobRecord>();
  if (isLoading) return <div className={styles.emptyState}>Carregando jobs…</div>;
  return (
    <>
      <div className={styles.jobDesktopGrid}><DesktopJobGrid /></div>
      <div className={styles.jobMobileList}>
        {data?.length
          ? data.map((record) => <TranslationJobCard key={record.id} record={record} />)
          : <div className={styles.emptyState}>Nenhum job encontrado.</div>}
      </div>
    </>
  );
};

export const TranslationJobList = () => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Pipeline multilíngue"
      title="Traduções em background"
      description="Controle a ordem operacional, remova tarefas pendentes e ajuste o orçamento global de geração."
    />
    <TranslationQueueControls />
    <div className={styles.historyHeader}>
      <h2>Histórico e diagnóstico</h2>
      <p>Registros concluídos, falhos e detalhes técnicos sanitizados.</p>
    </div>
    <List
      filters={filters}
      sort={{ field: "updated_at", order: "DESC" }}
      perPage={25}
      title="Traduções em background"
    >
      <ResponsiveJobList />
    </List>
  </div>
);
