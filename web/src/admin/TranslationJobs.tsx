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
} from "react-admin";
import { PageHeader, StatusBadge } from "./AdminUI";
import styles from "./Admin.module.css";

type TranslationJobRecord = {
  status: string;
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
  <TextInput key="language_code" source="language_code" label="Idioma" alwaysOn />,
  <TextInput key="content_id" source="content_id" label="ID do artigo" />,
];

const statusBadge = (status: string) => {
  if (status === "completed") return <StatusBadge label="Concluída" tone="success" />;
  if (status === "processing") return <StatusBadge label="Processando" tone="warning" />;
  if (status === "failed") return <StatusBadge label="Falhou" tone="danger" />;
  return <StatusBadge label="Pendente" tone="neutral" />;
};

export const TranslationJobList = () => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Pipeline multilíngue"
      title="Traduções em background"
      description="Acompanhe fila, processamento, tentativas, conclusões e falhas das traduções persistidas."
    />
    <List
      filters={filters}
      sort={{ field: "updated_at", order: "DESC" }}
      perPage={25}
      title="Traduções em background"
    >
      <Datagrid bulkActionButtons={false}>
        <FunctionField
          label="Estado"
          render={(record: TranslationJobRecord) => statusBadge(record.status)}
        />
        <TextField source="language_code" label="Idioma" />
        <TextField source="content_id" label="Artigo" />
        <NumberField source="attempts" label="Tentativas" />
        <DateField source="next_attempt_at" label="Próxima tentativa" showTime />
        <DateField source="started_at" label="Iniciada" showTime />
        <DateField source="completed_at" label="Concluída" showTime />
        <FunctionField
          label="Último erro"
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
    </List>
  </div>
);
