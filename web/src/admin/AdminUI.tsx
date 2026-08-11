import React from "react";
import { Title } from "react-admin";
import styles from "./Admin.module.css";

type PageHeaderProps = {
  eyebrow: string;
  title: string;
  description: string;
  action?: React.ReactNode;
};

export const PageHeader = ({
  eyebrow,
  title,
  description,
  action,
}: PageHeaderProps) => (
  <div className={styles.pageHeader}>
    <Title title={title} />
    <div className={styles.pageHeaderCopy}>
      <div className={styles.eyebrow}>{eyebrow}</div>
      <h1>{title}</h1>
      <p>{description}</p>
    </div>
    {action}
  </div>
);

type StatusTone = "success" | "warning" | "danger" | "info" | "neutral";

const toneClasses: Record<StatusTone, string> = {
  success: styles.statusSuccess,
  warning: styles.statusWarning,
  danger: styles.statusDanger,
  info: styles.statusInfo,
  neutral: styles.statusNeutral,
};

export const StatusBadge = ({
  label,
  tone = "neutral",
}: {
  label: string;
  tone?: StatusTone;
}) => (
  <span className={`${styles.status} ${toneClasses[tone]}`}>{label}</span>
);

export const formatNumber = (value: number | null | undefined) =>
  new Intl.NumberFormat("pt-BR").format(value ?? 0);

export const formatDateTime = (value: string | Date) =>
  new Intl.DateTimeFormat("pt-BR", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
