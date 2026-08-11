import React, { useCallback, useEffect, useState } from "react";
import {
  Datagrid,
  DateField,
  FunctionField,
  List,
  NumberField,
  TextField,
  TextInput,
  useRecordContext,
} from "react-admin";
import { PageHeader, StatusBadge, formatNumber } from "./AdminUI";
import styles from "./Admin.module.css";

type NotFoundStats = {
  uniqueUrls: number;
  unresolvedUrls: number;
  totalHits: number;
  generatedUrls: number;
};

type NotFoundRecord = {
  id: string;
  url: string;
  generated_slug?: string | null;
  generated_at?: string | null;
};

const filters = [
  <TextInput key="url" source="url" label="URL" alwaysOn />,
  <TextInput
    key="generated_slug"
    source="generated_slug"
    label="Slug gerado"
  />,
];

const Stats = () => {
  const [stats, setStats] = useState<NotFoundStats | null>(null);
  const [failed, setFailed] = useState(false);

  const loadStats = useCallback(async () => {
    setFailed(false);
    try {
      const response = await fetch("/api/admin/not-found-stats");
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      setStats((await response.json()) as NotFoundStats);
    } catch (error) {
      console.error("Failed to load 404 statistics", error);
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void loadStats();
  }, [loadStats]);

  if (failed) {
    return (
      <div className={styles.formIntro}>
        Os indicadores não puderam ser carregados. A lista abaixo continua
        disponível.
      </div>
    );
  }

  const items = [
    ["Acessos em URLs ausentes", stats?.totalHits],
    ["URLs únicas", stats?.uniqueUrls],
    ["Aguardando ação", stats?.unresolvedUrls],
    ["Artigos gerados", stats?.generatedUrls],
  ] as const;

  return (
    <div className={styles.statsStrip} aria-busy={!stats}>
      {items.map(([label, value]) => (
        <div className={styles.statItem} key={label}>
          <span>{label}</span>
          <strong>{stats ? formatNumber(value) : "—"}</strong>
        </div>
      ))}
    </div>
  );
};

const UrlField = (_props?: { source?: string; label?: string }) => {
  const record = useRecordContext<NotFoundRecord>();
  if (!record) return null;
  return (
    <a
      className={styles.textLink}
      href={record.url}
      target="_blank"
      rel="noreferrer"
      onClick={(event) => event.stopPropagation()}
    >
      {record.url}
    </a>
  );
};

const GenerateButton = (_props?: { label?: string }) => {
  const record = useRecordContext<NotFoundRecord>();
  if (!record) return null;
  const generated = Boolean(record.generated_at && record.generated_slug);
  const destination = generated
    ? `/content/${encodeURIComponent(record.generated_slug as string)}`
    : `/create?url=${encodeURIComponent(record.url)}`;

  return (
    <a
      href={destination}
      className={`${styles.actionButton} ${
        generated ? "" : styles.actionButtonPrimary
      }`}
    >
      {generated ? "Abrir artigo" : record.generated_slug ? "Tentar novamente" : "Gerar artigo"}
    </a>
  );
};

export const NotFoundRequestList = () => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Oportunidades"
      title="Rotas 404"
      description="Transforme demanda real em conteúdo: priorize URLs recorrentes e gere um artigo diretamente para o endereço esperado."
    />
    <Stats />
    <List
      filters={filters}
      sort={{ field: "last_seen_at", order: "DESC" }}
      perPage={25}
      title="Rotas 404"
    >
      <Datagrid bulkActionButtons={false}>
        <UrlField source="url" label="URL" />
        <NumberField source="hit_count" label="Acessos" />
        <DateField source="last_seen_at" label="Último acesso" showTime />
        <FunctionField
          source="generated_at"
          label="Situação"
          render={(record: NotFoundRecord) =>
            record.generated_at ? (
              <StatusBadge label="Resolvida" tone="success" />
            ) : record.generated_slug ? (
              <StatusBadge label="Falhou" tone="danger" />
            ) : (
              <StatusBadge label="Pendente" tone="warning" />
            )
          }
        />
        <TextField source="generated_slug" label="Slug gerado" />
        <GenerateButton />
      </Datagrid>
    </List>
  </div>
);
