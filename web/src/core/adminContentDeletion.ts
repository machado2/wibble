import type { Prisma } from "@prisma/client";

type ContentDeletionClient = Pick<
  Prisma.TransactionClient,
  "content" | "content_vote"
>;

export const deleteContentWithRelations = async (
  transaction: ContentDeletionClient,
  id: string
) => {
  await transaction.content_vote.deleteMany({
    where: { content_id: id },
  });

  return transaction.content.delete({
    where: { id },
  });
};

export const deleteManyContentWithRelations = async (
  transaction: ContentDeletionClient,
  ids: string[]
) => {
  await transaction.content_vote.deleteMany({
    where: { content_id: { in: ids } },
  });
  await transaction.content.deleteMany({
    where: { id: { in: ids } },
  });

  return ids;
};
