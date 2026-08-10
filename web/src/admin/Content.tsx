// src/admin/Content.tsx
import React, { useEffect } from "react";
import {
  List,
  Edit,
  Create,
  SimpleForm,
  TextInput,
  DateInput,
  BooleanInput,
  TextField,
  DateField,
  BooleanField,
  Datagrid,
  useRecordContext,
  NumberField,
  DateTimeInput,
} from "react-admin";
import { content } from "@prisma/client";
import { BoxedImage } from "@/components/BoxedImage";
import styles from "./Content.module.css";
import { v4 as uuidv4 } from "uuid";
import slugify from "slugify";
import { useWatch, useFormContext } from "react-hook-form";

const contentFilters = [
  <TextInput key={1} source="title" alwaysOn={true} />,
  <TextInput key={2} source="content" alwaysOn={true} />,
  <TextInput key={3} source="slug" alwaysOn={true} />,
  <TextInput key={4} source="model" alwaysOn={true} />,
];

const ImageThumbnailField = () => {
  const record = useRecordContext<content>();
  const [, setImageTimestamp] = React.useState(Date.now());

  React.useEffect(() => {
    setImageTimestamp(Date.now());
  }, [record]);

  return record.image_prompt ? (
    <BoxedImage
      alt={record.image_prompt}
      prompt={record.image_prompt}
      className={styles.imagethumb}
    />
  ) : (
    <p>null</p>
  );
};

const LinkContent = (props) => {
  const record = useRecordContext(props);

  const handleClick = (e) => {
    // Prevent React Admin from capturing the click event
    e.stopPropagation();
  };

  return (
    <a
      href={`${window.location.origin}/${record?.slug}`}
      onClick={handleClick}
      target="_blank"
      rel="noopener noreferrer"
    >
      {record?.title}
    </a>
  );
};

export const ContentList = () => (
  <List filters={contentFilters}>
    <Datagrid rowClick="edit">
      <BooleanField source="flagged" />
      <ImageThumbnailField />
      <LinkContent />
      <BooleanField source="generating" />
      <DateField source="created_at" />
      <NumberField source="votes" />
      <NumberField source="view_count" />
      <TextField source="user_email" />
      <TextField source="user_input" />
      <NumberField source="hot_score" />
      <TextField source="model" />
    </Datagrid>
  </List>
);

const SlugInputEdit = (props: any) => {
  const watchTitle = (useWatch({ name: "title" }) as string) ?? "";
  const watchSlug = (useWatch({ name: "slug" }) as string) ?? "";
  const form = useFormContext();

  useEffect(() => {
    const slug = slugify(watchTitle, { lower: true, strict: false });
    console.log(slug);
    if (slug != watchSlug) {
      form.setValue("slug", slug);
    }
  }, [watchTitle]);
  return <TextInput source="slug" {...props} />;
};

export const ContentCreate = (props: any) => {
  return (
    <Create {...props}>
      <SimpleForm>
        <SlugInputEdit />
        <TextInput source="title" fullWidth />
        <TextInput source="description" multiline fullWidth />
        <TextInput source="image_prompt" multiline fullWidth />
        <TextInput source="content" multiline fullWidth />
        <BooleanInput source="generating" disabled defaultValue={false} />

        <TextInput source="id" defaultValue={uuidv4()} disabled />
        <BooleanInput source="flagged" defaultValue={false} />
        <TextInput source="user_input" defaultValue="n/a" multiline fullWidth />
      </SimpleForm>
    </Create>
  );
};

export const ContentEdit = (props: any) => (
  <Edit {...props}>
    <SimpleForm>
      <BooleanInput source="flagged" />
      <TextInput source="image_prompt" multiline fullWidth />
      <ImageThumbnailField />
      <TextInput disabled source="slug" />
      <DateTimeInput source="created_at" />
      <TextInput source="ip_address" />
      <DateInput source="generation_finished_at" />
      <TextInput source="title" fullWidth />
      <TextInput source="description" multiline fullWidth />
      <TextInput source="content" multiline fullWidth />
      <TextInput source="user_input" multiline fullWidth />
      <TextInput source="model" />
    </SimpleForm>
  </Edit>
);
