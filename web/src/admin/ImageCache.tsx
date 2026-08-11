import React, { useState } from "react";
import Image from "next/image";
import {
  BooleanInput,
  Datagrid,
  DateField,
  DateTimeInput,
  Edit,
  List,
  NumberField,
  NumberInput,
  SaveButton,
  SimpleForm,
  TextField,
  TextInput,
  Toolbar,
  useRecordContext,
} from "react-admin";
import { image_cache } from "@prisma/client";
import { PageHeader, StatusBadge } from "./AdminUI";
import styles from "./Admin.module.css";

const imageFilters = [
  <TextInput key="id" label="ID" source="id" alwaysOn />,
  <TextInput key="prompt" label="Prompt" source="prompt" alwaysOn />,
  <TextInput key="status" label="Status" source="status" />,
  <TextInput key="model" label="Modelo" source="model" />,
];

const imageStatus = (status: string) => {
  const normalized = status.toLowerCase();
  if (normalized === "completed")
    return <StatusBadge label="Concluída" tone="success" />;
  if (normalized.includes("fail") || normalized.includes("error"))
    return <StatusBadge label={status} tone="danger" />;
  if (normalized.includes("generat") || normalized.includes("process"))
    return <StatusBadge label={status} tone="warning" />;
  return <StatusBadge label={status} tone="neutral" />;
};

const ImageStatusField = (_props?: { source?: string; label?: string }) => {
  const record = useRecordContext<image_cache>();
  return record ? imageStatus(record.status) : null;
};

const ImageApiField = ({
  preview = false,
}: {
  preview?: boolean;
  source?: string;
  label?: string;
}) => {
  const record = useRecordContext<image_cache>();
  const [timestamp] = useState(Date.now());
  if (!record) return null;

  return (
    <Image
      src={
        record.status === "completed"
          ? `/api/image/${record.id}?v=${timestamp}`
          : "/placeholder.jpeg"
      }
      alt={record.alt_text || record.prompt}
      title={record.alt_text || record.prompt}
      className={preview ? styles.imagePreview : styles.thumbnail}
      width={preview ? 520 : 54}
      height={preview ? 390 : 42}
      unoptimized
    />
  );
};

const ImageEditToolbar = () => (
  <Toolbar className={styles.toolbar}>
    <SaveButton label="Salvar imagem" />
  </Toolbar>
);

export const ImageCacheList = (props: Record<string, unknown>) => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Mídia"
      title="Imagens"
      description="Acompanhe a fila de geração, localize falhas e ajuste prompts sem perder de vista uso e proveniência."
    />
    <List
      filters={imageFilters}
      sort={{ field: "created_at", order: "DESC" }}
      perPage={25}
      title="Imagens"
      {...props}
    >
      <Datagrid rowClick="edit" bulkActionButtons={false}>
        <ImageApiField source="id" label="Imagem" />
        <ImageStatusField source="status" label="Estado" />
        <TextField source="prompt" label="Prompt" />
        <TextField source="model" label="Modelo" />
        <DateField source="created_at" label="Criada em" showTime />
        <NumberField source="view_count" label="Views" />
        <NumberField source="fail_count" label="Falhas" />
      </Datagrid>
    </List>
  </div>
);

export const ImageCacheEdit = (props: Record<string, unknown>) => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Mídia"
      title="Detalhes da imagem"
      description="Inspecione o resultado, o prompt e os dados técnicos usados pelo gerador."
    />
    <Edit {...props} title="Detalhes da imagem" mutationMode="pessimistic">
      <SimpleForm toolbar={<ImageEditToolbar />}>
        <div className={styles.formGrid}>
          <section className={styles.formSection}>
            <h2>Resultado</h2>
            <p className={styles.formSectionDescription}>
              Prévia e instruções editoriais da imagem.
            </p>
            <ImageApiField preview />
            <div className={styles.formDivider} />
            <TextInput source="id" label="ID" disabled fullWidth />
            <TextInput
              source="prompt"
              label="Prompt"
              multiline
              minRows={5}
              fullWidth
            />
            <TextInput
              source="alt_text"
              label="Texto alternativo"
              multiline
              minRows={3}
              fullWidth
            />
          </section>
          <aside className={styles.formSection}>
            <h2>Processamento</h2>
            <p className={styles.formSectionDescription}>
              Status, parâmetros e rastreabilidade técnica.
            </p>
            <TextInput source="status" label="Status" fullWidth />
            <TextInput source="model" label="Modelo" fullWidth />
            <TextInput source="generator" label="Gerador" fullWidth />
            <TextInput source="seed" label="Seed" fullWidth />
            <TextInput
              source="parameters"
              label="Parâmetros"
              multiline
              minRows={4}
              fullWidth
            />
            <NumberInput source="fail_count" label="Falhas" fullWidth />
            <NumberInput source="view_count" label="Views" fullWidth />
            <BooleanInput source="flagged" label="Sinalizada" />
            <BooleanInput source="regenerate" label="Regenerar" />
            <DateTimeInput source="created_at" label="Criada em" fullWidth />
            <DateTimeInput
              source="generation_started_at"
              label="Geração iniciada em"
              fullWidth
            />
            <DateTimeInput
              source="generation_finished_at"
              label="Geração concluída em"
              fullWidth
            />
            <TextInput
              source="last_error"
              label="Último erro"
              multiline
              minRows={4}
              fullWidth
            />
          </aside>
        </div>
      </SimpleForm>
    </Edit>
  </div>
);
