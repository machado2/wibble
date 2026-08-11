import React from "react";
import {
  Datagrid,
  DateField,
  FunctionField,
  List,
  NumberField,
  TextField,
  TextInput,
} from "react-admin";
import { PageHeader, StatusBadge } from "./AdminUI";
import styles from "./Admin.module.css";

type SearchRecord = {
  term: string;
  result_count: number;
};

type FailureRecord = {
  reason?: string | null;
  exception?: string | null;
};

const searchFilters = [
  <TextInput key="term" source="term" label="Termo" alwaysOn />,
];

const failureFilters = [
  <TextInput key="slug" source="slug" label="Slug" alwaysOn />,
  <TextInput key="reason" source="reason" label="Motivo" />,
];

export const SearchHistoryList = () => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Audiência"
      title="Histórico de buscas"
      description="Entenda o que as pessoas procuram e onde o catálogo ainda não entrega respostas."
    />
    <List
      filters={searchFilters}
      sort={{ field: "created_at", order: "DESC" }}
      perPage={50}
      title="Histórico de buscas"
    >
      <Datagrid bulkActionButtons={false}>
        <TextField source="term" label="Termo pesquisado" />
        <FunctionField
          label="Resultado"
          render={(record: SearchRecord) =>
            record.result_count > 0 ? (
              <StatusBadge
                label={`${record.result_count} resultado${
                  record.result_count === 1 ? "" : "s"
                }`}
                tone="success"
              />
            ) : (
              <StatusBadge label="Sem resultado" tone="danger" />
            )
          }
        />
        <DateField source="created_at" label="Buscado em" showTime />
        <NumberField source="result_count" label="Qtd." />
      </Datagrid>
    </List>
  </div>
);

export const GenerationFailureList = () => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Operação"
      title="Falhas de geração"
      description="Registros técnicos do pipeline para localizar padrões, artigos afetados e exceções recorrentes."
    />
    <List
      filters={failureFilters}
      sort={{ field: "created_at", order: "DESC" }}
      perPage={25}
      title="Falhas de geração"
    >
      <Datagrid bulkActionButtons={false}>
        <FunctionField
          label="Estado"
          render={() => <StatusBadge label="Falha" tone="danger" />}
        />
        <TextField source="slug" label="Slug" />
        <TextField source="reason" label="Motivo" />
        <FunctionField
          label="Exceção"
          render={(record: FailureRecord) => {
            const value = record.exception || "—";
            return (
              <span className={styles.cellMeta} title={value}>
                {value}
              </span>
            );
          }}
        />
        <DateField source="created_at" label="Ocorrida em" showTime />
      </Datagrid>
    </List>
  </div>
);
