// src/admin/ImageCache.tsx
import React from "react";
import {
  List,
  Edit,
  SimpleForm,
  TextInput,
  DateInput,
  BooleanInput,
  TextField,
  DateField,
  BooleanField,
  Datagrid,
  useUpdate,
  useRecordContext,
  NumberField,
  NumberInput,
} from "react-admin";
import { Button } from "antd";
import { Image } from "antd";
import { image_cache } from "@prisma/client";

const ImageApiField = () => {
  const record = useRecordContext<image_cache>();
  const [imageTimestamp, setImageTimestamp] = React.useState(Date.now());

  React.useEffect(() => {
    setImageTimestamp(Date.now());
  }, [record]);

  return record.status === "completed" ? (
    <Image src={`/api/image/${record.id}?v=${imageTimestamp}`} width={100} />
  ) : (
    <p>null</p>
  );
};

const contentFilters = [
  <TextInput key={1} label="id" source="id" alwaysOn={true} />,
  <TextInput key={2} label="prompt" source="prompt" alwaysOn={true} />,
];

export const ImageCacheList: React.FC = (props) => (
  <List filters={contentFilters} {...props}>
    <Datagrid rowClick="edit">
      <ImageApiField />
      <TextField source="prompt" />
      <TextField source="model" />
      <TextField source="seed" />
      <DateField source="created_at" />
      <NumberField source="view_count" />
      <NumberField source="fail_count" />
      <BooleanField source="flagged" />
    </Datagrid>
  </List>
);

export const ImageCacheEdit: React.FC = (props) => {
  const record = useRecordContext<image_cache>();
  const [update, { isLoading }] = useUpdate<image_cache>();

  const handleClearImage = () => {
    update("image_cache", {
      id: record.id,
      data: { regenerate: true, status: "pending" },
    });
  };

  return (
    <Edit {...props}>
      <SimpleForm>
        <TextInput disabled source="id" />
        <TextInput source="prompt" multiline fullWidth />
        <TextInput source="parameters" multiline fullWidth />
        <TextInput source="model" />
        <NumberInput source="seed" />
        <BooleanInput source="regenerate" />
        <TextInput source="generator" />
        <NumberInput source="fail_count" />
        <NumberInput source="view_count" />
        <ImageApiField />
        <Button onClick={handleClearImage} loading={isLoading}>
          Clear Image
        </Button>

        <DateInput source="created_at" />
        <BooleanInput source="flagged" />
      </SimpleForm>
    </Edit>
  );
};
