import React from "react";
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
      { id: "Limite temporário de traduções", name: "Aguardando quota" },
      { id: "Falha temporária ao gerar tradução", name: "Falha temporária" },
      { id: "Falha ao gerar tradução", name: "Falha definitiva" },
    ]}
  />,
  <TextInput key="language_code" source="language_code" label="Idioma" alwaysOn />,
  <TextInput key="content_id" source="content_id" label="ID do artigo" />,
];

const statusBadge = (record: Pick<TranslationJobRecord, "status" | "last_error">) => {
  if (
    record.status === "pending" &&
    record.last_error === "Limite temporário de traduções"
  ) {
    return <StatusBadge label="Aguardando quota" tone="info" />;
  }
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
      description="Fila somente leitura com espera por quota, tentativas, leases, timestamps e erros operacionais sanitizados."
    />
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
