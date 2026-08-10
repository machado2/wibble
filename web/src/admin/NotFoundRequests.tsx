import React, { useEffect, useState } from "react";
import {
  Datagrid,
  DateField,
  FunctionField,
  List,
  NumberField,
  TextField,
  useRecordContext,
} from "react-admin";
import { Button, Card, Col, Row, Statistic } from "antd";

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

const Stats = () => {
  const [stats, setStats] = useState<NotFoundStats | null>(null);

  useEffect(() => {
    void fetch("/api/admin/not-found-stats")
      .then((response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        return response.json() as Promise<NotFoundStats>;
      })
      .then(setStats)
      .catch((error) => console.error("Failed to load 404 statistics", error));
  }, []);

  return (
    <Card style={{ marginBottom: 16 }} loading={!stats}>
      <Row gutter={16}>
        <Col xs={24} sm={12} lg={6}><Statistic title="404 hits" value={stats?.totalHits ?? 0} /></Col>
        <Col xs={24} sm={12} lg={6}><Statistic title="Unique URLs" value={stats?.uniqueUrls ?? 0} /></Col>
        <Col xs={24} sm={12} lg={6}><Statistic title="Waiting" value={stats?.unresolvedUrls ?? 0} /></Col>
        <Col xs={24} sm={12} lg={6}><Statistic title="Generated" value={stats?.generatedUrls ?? 0} /></Col>
      </Row>
    </Card>
  );
};

const UrlField = () => {
  const record = useRecordContext<NotFoundRecord>();
  if (!record) return null;
  return (
    <a href={record.url} target="_blank" rel="noreferrer" onClick={(event) => event.stopPropagation()}>
      {record.url}
    </a>
  );
};

const GenerateButton = () => {
  const record = useRecordContext<NotFoundRecord>();
  if (!record) return null;
  const destination = record.generated_at && record.generated_slug
    ? `/content/${encodeURIComponent(record.generated_slug)}`
    : `/create?url=${encodeURIComponent(record.url)}`;
  return (
    <Button href={destination} type={record.generated_at ? "default" : "primary"}>
      {record.generated_at
        ? "Open article"
        : record.generated_slug
          ? "Retry / inspect"
          : "Generate"}
    </Button>
  );
};

export const NotFoundRequestList = () => (
  <>
    <Stats />
    <List sort={{ field: "last_seen_at", order: "DESC" }}>
      <Datagrid bulkActionButtons={false}>
        <UrlField />
        <NumberField source="hit_count" label="Hits" />
        <DateField source="first_seen_at" showTime />
        <DateField source="last_seen_at" showTime />
        <FunctionField
          source="generated_at"
          label="Generated"
          render={(record: NotFoundRecord) => record.generated_at ? "Yes" : "No"}
        />
        <TextField source="generated_slug" />
        <GenerateButton />
      </Datagrid>
    </List>
  </>
);
