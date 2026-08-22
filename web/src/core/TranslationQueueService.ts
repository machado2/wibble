import { Prisma } from "@prisma/client";
import { v4 as uuidv4 } from "uuid";
import prisma from "./PrismaWibble";
import {
  ArticleTranslationError,
  ArticleTranslationService,
} from "./ArticleTranslationService";
import { resolveSupportedTranslationLanguage } from "./translationLanguages";
import { generationSafetyIdentifier } from "./generationIdentity";

const MAX_VISIBLE_BATCH = 20;
const MAX_ATTEMPTS = 3;
export const BACKGROUND_TRANSLATION_IDENTITY = "background-translations@wibble.internal";

export type TranslationQueueJob = {
  id: string;
  slug: string;
  languageCode: string;
  requestedBy: string;
  attempts: number;
  leaseId: string;
};

type QueueRepository = {
  enqueue(slugs: string[], languageCode: string, requestedBy: string): Promise<number>;
  claimNext(): Promise<TranslationQueueJob | null>;
  complete(id: string, leaseId: string): Promise<void>;
  retry(
    id: string,
    leaseId: string,
    retryAfterSeconds: number,
    error: string,
    undoAttempt?: boolean
  ): Promise<void>;
  fail(id: string, leaseId: string, error: string): Promise<void>;
};

type Translator = Pick<ArticleTranslationService, "generate">;
type Dependencies = { repository: QueueRepository; translator: Translator };

type ClaimedRow = {
  id: string;
  slug: string;
  language_code: string;
  requested_by: string;
  attempts: number;
  lease_id: string;
};

export class PrismaTranslationQueueRepository implements QueueRepository {
  async enqueue(slugs: string[], languageCode: string, requestedBy: string): Promise<number> {
    if (slugs.length === 0) return 0;
    const rows = await prisma.$queryRaw<Array<{ id: string }>>(Prisma.sql`
      SELECT c.id
      FROM content c
      WHERE c.slug IN (${Prisma.join(slugs)})
        AND c.published = true
        AND c.flagged = false
        AND c.content IS NOT NULL
        AND NOT EXISTS (
          SELECT 1 FROM content_translation t
          WHERE t.content_id = c.id AND t.language_code = ${languageCode}
        )
      LIMIT ${MAX_VISIBLE_BATCH}
    `);

    let queued = 0;
    for (const row of rows) {
      const changed = await prisma.$executeRaw`
        INSERT INTO translation_job (
          id, content_id, language_code, requested_by, status,
          attempts, created_at, updated_at, next_attempt_at
        ) VALUES (
          ${uuidv4()}, ${row.id}, ${languageCode}, ${requestedBy}, 'pending',
          0, NOW(), NOW(), DATE_TRUNC('second', NOW())
        )
        ON CONFLICT (content_id, language_code) DO NOTHING
      `;
      queued += Number(changed > 0);
    }
    return queued;
  }

  async claimNext(): Promise<TranslationQueueJob | null> {
    const leaseId = uuidv4();
    return prisma.$transaction(async (tx) => {
      const rows = await tx.$queryRaw<ClaimedRow[]>(Prisma.sql`
        WITH candidate AS (
          SELECT j.id
          FROM translation_job j
          WHERE (
            j.status = 'pending' AND j.next_attempt_at <= NOW()
          ) OR (
            j.status = 'processing' AND j.started_at < NOW() - INTERVAL '15 minutes'
          )
          ORDER BY j.created_at ASC
          FOR UPDATE SKIP LOCKED
          LIMIT 1
        )
        UPDATE translation_job j
        SET status = 'processing',
            lease_id = ${leaseId},
            attempts = j.attempts + 1,
            started_at = NOW(),
            completed_at = NULL,
            updated_at = NOW(),
            last_error = NULL
        FROM candidate, content c
        WHERE j.id = candidate.id AND c.id = j.content_id
        RETURNING j.id, c.slug, j.language_code, j.requested_by, j.attempts, j.lease_id
      `);
      const row = rows[0];
      return row
        ? {
            id: row.id,
            slug: row.slug,
            languageCode: row.language_code,
            requestedBy: row.requested_by,
            attempts: row.attempts,
            leaseId: row.lease_id,
          }
        : null;
    });
  }

  async complete(id: string, leaseId: string): Promise<void> {
    const changed = await prisma.$executeRaw`
      UPDATE translation_job
      SET status = 'completed', completed_at = NOW(), updated_at = NOW(),
          last_error = NULL, lease_id = NULL
      WHERE id = ${id} AND status = 'processing' AND lease_id = ${leaseId}
    `;
    if (changed !== 1) throw new Error("Translation job lease was lost");
  }

  async retry(
    id: string,
    leaseId: string,
    retryAfterSeconds: number,
    error: string,
    undoAttempt = false
  ): Promise<void> {
    const seconds = Math.max(1, Math.min(86_400, Math.ceil(retryAfterSeconds)));
    const changed = await prisma.$executeRaw`
      UPDATE translation_job
      SET status = 'pending',
          attempts = CASE WHEN ${undoAttempt} THEN GREATEST(attempts - 1, 0) ELSE attempts END,
          next_attempt_at = NOW() + (${seconds} * INTERVAL '1 second'),
          updated_at = NOW(), last_error = ${error}, lease_id = NULL
      WHERE id = ${id} AND status = 'processing' AND lease_id = ${leaseId}
    `;
    if (changed !== 1) throw new Error("Translation job lease was lost");
  }

  async fail(id: string, leaseId: string, error: string): Promise<void> {
    const changed = await prisma.$executeRaw`
      UPDATE translation_job
      SET status = 'failed', completed_at = NOW(), updated_at = NOW(),
          last_error = ${error}, lease_id = NULL
      WHERE id = ${id} AND status = 'processing' AND lease_id = ${leaseId}
    `;
    if (changed !== 1) throw new Error("Translation job lease was lost");
  }
}

export class TranslationQueueService {
  private readonly repository: QueueRepository;
  private readonly translator: Translator;

  constructor(dependencies?: Partial<Dependencies>) {
    this.repository = dependencies?.repository ?? new PrismaTranslationQueueRepository();
    this.translator = dependencies?.translator ?? new ArticleTranslationService();
  }

  async enqueueVisible(
    requestedSlugs: string[],
    requestedLanguage: string,
    requestedBy: string
  ): Promise<number> {
    const languageCode = resolveSupportedTranslationLanguage(requestedLanguage);
    const slugs = Array.from(
      new Set(requestedSlugs.map((slug) => slug.trim().toLowerCase()).filter(Boolean))
    ).slice(0, MAX_VISIBLE_BATCH);
    return this.repository.enqueue(slugs, languageCode, requestedBy.trim().toLowerCase());
  }

  async processNext(): Promise<
    | { processed: false }
    | { processed: true; status: "completed" | "pending" | "failed" }
  > {
    const job = await this.repository.claimNext();
    if (!job) return { processed: false };

    try {
      const requestedBy = job.requestedBy.toLowerCase();
      await this.translator.generate(
        job.slug,
        job.languageCode,
        requestedBy,
        generationSafetyIdentifier(requestedBy, "background-worker")
      );
      await this.repository.complete(job.id, job.leaseId);
      return { processed: true, status: "completed" };
    } catch (error) {
      if (error instanceof ArticleTranslationError && error.statusCode === 429) {
        await this.repository.retry(
          job.id,
          job.leaseId,
          error.retryAfterSeconds ?? 60,
          "Limite temporário de traduções",
          true
        );
        return { processed: true, status: "pending" };
      }
      if (job.attempts < MAX_ATTEMPTS) {
        await this.repository.retry(
          job.id,
          job.leaseId,
          60 * 2 ** Math.max(0, job.attempts - 1),
          "Falha temporária ao gerar tradução"
        );
        return { processed: true, status: "pending" };
      }
      await this.repository.fail(job.id, job.leaseId, "Falha ao gerar tradução");
      return { processed: true, status: "failed" };
    }
  }
}
