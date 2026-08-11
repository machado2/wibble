import React, { useEffect, useMemo } from "react";
import Image from "next/image";
import {
  BooleanInput,
  Create,
  Datagrid,
  DateField,
  DateTimeInput,
  DeleteButton,
  Edit,
  FunctionField,
  List,
  NumberField,
  SaveButton,
  SelectInput,
  SimpleForm,
  TextField,
  TextInput,
  Toolbar,
  required,
  useRecordContext,
} from "react-admin";
import { content } from "@prisma/client";
import { BoxedImage } from "@/components/BoxedImage";
import { v4 as uuidv4 } from "uuid";
import slugify from "slugify";
import { useFormContext, useWatch } from "react-hook-form";
import { PageHeader, StatusBadge } from "./AdminUI";
import styles from "./Admin.module.css";

const contentFilters = [
  <TextInput key="title" source="title" label="Título" alwaysOn />,
  <TextInput key="slug" source="slug" label="Slug" />,
  <TextInput key="content" source="content" label="Texto" />,
  <TextInput key="model" source="model" label="Modelo" />,
  <SelectInput
    key="published"
    source="published"
    label="Publicação"
    choices={[
      { id: true, name: "Publicado" },
      { id: false, name: "Rascunho" },
    ]}
  />,
  <SelectInput
    key="flagged"
    source="flagged"
    label="Moderação"
    choices={[
      { id: true, name: "Sinalizado" },
      { id: false, name: "Normal" },
    ]}
  />,
];

const ImageThumbnailField = (_props?: { source?: string; label?: string }) => {
  const record = useRecordContext<content>();

  if (!record?.image_prompt) {
    return (
      <Image
        className={styles.thumbnail}
        src="/placeholder.jpeg"
        alt="Artigo sem imagem"
        width={54}
        height={42}
      />
    );
  }

  return (
    <BoxedImage
      alt={record.image_prompt}
      prompt={record.image_prompt}
      className={styles.thumbnail}
    />
  );
};

const ArticleLinkField = (_props?: { source?: string; label?: string }) => {
  const record = useRecordContext<content>();
  if (!record) return null;

  return (
    <a
      className={styles.articleLink}
      href={`/${record.slug}`}
      onClick={(event) => event.stopPropagation()}
      target="_blank"
      rel="noopener noreferrer"
      title={record.title}
    >
      {record.title}
    </a>
  );
};

const ArticleStatusField = (_props?: { source?: string; label?: string }) => {
  const record = useRecordContext<content>();
  if (!record) return null;
  if (record.flagged) return <StatusBadge label="Sinalizado" tone="danger" />;
  if (record.generating)
    return <StatusBadge label="Gerando" tone="warning" />;
  if (record.published)
    return <StatusBadge label="Publicado" tone="success" />;
  return <StatusBadge label="Rascunho" tone="neutral" />;
};

export const ContentList = () => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Conteúdo"
      title="Artigos"
      description="Encontre, revise e publique histórias. O estado de cada item aparece antes dos números para deixar as prioridades óbvias."
    />
    <List
      filters={contentFilters}
      sort={{ field: "created_at", order: "DESC" }}
      perPage={25}
      title="Artigos"
    >
      <Datagrid
        rowClick="edit"
        bulkActionButtons={false}
        sx={{
          "& .column-created_at, & .column-votes, & .column-view_count, & .column-model": {
            display: { xs: "none", md: "table-cell" },
          },
          "& .column-image_prompt": {
            display: { xs: "none", sm: "table-cell" },
          },
        }}
      >
        <ImageThumbnailField source="image_prompt" label="Imagem" />
        <ArticleStatusField source="status" label="Estado" />
        <ArticleLinkField source="title" label="Artigo" />
        <DateField source="created_at" label="Criado em" showTime />
        <NumberField source="votes" label="Votos" />
        <NumberField source="view_count" label="Views" />
        <TextField source="model" label="Modelo" />
      </Datagrid>
    </List>
  </div>
);

const SlugInputCreate = () => {
  const title = (useWatch({ name: "title" }) as string) ?? "";
  const currentSlug = (useWatch({ name: "slug" }) as string) ?? "";
  const form = useFormContext();

  useEffect(() => {
    const nextSlug = slugify(title, { lower: true, strict: true });
    if (nextSlug !== currentSlug) form.setValue("slug", nextSlug);
  }, [currentSlug, form, title]);

  return (
    <TextInput
      source="slug"
      label="Slug automático"
      fullWidth
      validate={required()}
      helperText="Criado a partir do título."
      InputProps={{ readOnly: true }}
    />
  );
};

const CreateToolbar = () => (
  <Toolbar className={styles.toolbar}>
    <SaveButton label="Criar artigo" />
  </Toolbar>
);

const EditToolbar = () => (
  <Toolbar className={styles.toolbar}>
    <SaveButton label="Salvar alterações" />
    <DeleteButton
      label="Excluir artigo"
      mutationMode="pessimistic"
      confirmTitle="Excluir este artigo?"
      confirmContent="O artigo e seus votos serão removidos de forma permanente. Esta ação não pode ser desfeita."
      redirect="list"
    />
  </Toolbar>
);

export const ContentCreate = (props: Record<string, unknown>) => {
  const defaultValues = useMemo(
    () => ({
      id: uuidv4(),
      flagged: false,
      generating: false,
      published: true,
      model: "manual",
      user_input: "Criação manual pelo painel administrativo",
    }),
    []
  );

  return (
    <div className={styles.page}>
      <PageHeader
        eyebrow="Conteúdo"
        title="Novo artigo"
        description="Crie uma publicação manual com os mesmos campos usados pelo pipeline de geração."
      />
      <Create {...props} title="Novo artigo" redirect="list">
        <SimpleForm
          defaultValues={defaultValues}
          toolbar={<CreateToolbar />}
        >
          <TextInput source="id" sx={{ display: "none" }} />
          <div className={styles.formIntro}>
            O artigo pode ser salvo já publicado ou como rascunho. Revise o slug e
            a descrição antes de concluir.
          </div>
          <div className={styles.formGrid}>
            <section className={styles.formSection}>
              <h2>História</h2>
              <p className={styles.formSectionDescription}>
                O que o leitor verá na página e nas listagens.
              </p>
              <TextInput
                source="title"
                label="Título"
                fullWidth
                validate={required()}
              />
              <SlugInputCreate />
              <TextInput
                source="description"
                label="Descrição"
                multiline
                minRows={3}
                fullWidth
                validate={required()}
              />
              <TextInput
                source="content"
                label="Conteúdo"
                multiline
                minRows={14}
                fullWidth
              />
              <TextInput
                source="image_prompt"
                label="Prompt da imagem"
                multiline
                minRows={3}
                fullWidth
              />
            </section>
            <aside className={styles.formSection}>
              <h2>Publicação</h2>
              <p className={styles.formSectionDescription}>
                Controle de visibilidade e proveniência.
              </p>
              <BooleanInput source="published" label="Publicado" />
              <BooleanInput source="flagged" label="Sinalizado" />
              <BooleanInput source="generating" label="Em geração" disabled />
              <div className={styles.formDivider} />
              <TextInput
                source="model"
                label="Modelo / origem"
                fullWidth
                validate={required()}
              />
              <TextInput
                source="user_input"
                label="Instrução original"
                multiline
                minRows={4}
                fullWidth
                validate={required()}
              />
            </aside>
          </div>
        </SimpleForm>
      </Create>
    </div>
  );
};

export const ContentEdit = (props: Record<string, unknown>) => (
  <div className={styles.page}>
    <PageHeader
      eyebrow="Conteúdo"
      title="Editar artigo"
      description="Revise texto, publicação e metadados. A exclusão exige confirmação e só conclui após resposta do servidor."
    />
    <Edit {...props} title="Editar artigo" mutationMode="pessimistic">
      <SimpleForm toolbar={<EditToolbar />}>
        <div className={styles.formGrid}>
          <section className={styles.formSection}>
            <h2>História</h2>
            <p className={styles.formSectionDescription}>
              Conteúdo editorial e imagem associada.
            </p>
            <TextInput
              source="title"
              label="Título"
              fullWidth
              validate={required()}
            />
            <TextInput source="slug" label="Slug" disabled fullWidth />
            <TextInput
              source="description"
              label="Descrição"
              multiline
              minRows={3}
              fullWidth
              validate={required()}
            />
            <TextInput
              source="content"
              label="Conteúdo"
              multiline
              minRows={16}
              fullWidth
            />
            <TextInput
              source="image_prompt"
              label="Prompt da imagem"
              multiline
              minRows={3}
              fullWidth
            />
            <ImageThumbnailField source="image_prompt" />
          </section>
          <aside className={styles.formSection}>
            <h2>Estado</h2>
            <p className={styles.formSectionDescription}>
              Publicação, moderação e rastreabilidade.
            </p>
            <BooleanInput source="published" label="Publicado" />
            <BooleanInput source="flagged" label="Sinalizado" />
            <BooleanInput source="generating" label="Em geração" />
            <div className={styles.formDivider} />
            <DateTimeInput source="created_at" label="Criado em" fullWidth />
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
            <TextInput source="model" label="Modelo" fullWidth />
            <TextInput source="user_email" label="E-mail do autor" fullWidth />
            <TextInput
              source="user_input"
              label="Instrução original"
              multiline
              minRows={4}
              fullWidth
            />
            <FunctionField
              label="Indicadores"
              render={(record: content) =>
                `${record.votes} votos · ${record.view_count} views · score ${
                  record.hot_score ?? 0
                }`
              }
            />
          </aside>
        </div>
      </SimpleForm>
    </Edit>
  </div>
);
