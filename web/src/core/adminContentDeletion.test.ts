import {
  deleteContentWithRelations,
  deleteManyContentWithRelations,
} from "./adminContentDeletion";

describe("admin content deletion", () => {
  test("deletes votes before deleting a single article", async () => {
    const calls: string[] = [];
    const deletedArticle = { id: "article-1", slug: "example" };
    const transaction = {
      content_vote: {
        deleteMany: jest.fn(async () => {
          calls.push("votes");
          return { count: 2 };
        }),
      },
      content: {
        delete: jest.fn(async () => {
          calls.push("content");
          return deletedArticle;
        }),
        deleteMany: jest.fn(),
      },
    };

    await expect(
      deleteContentWithRelations(transaction as never, "article-1")
    ).resolves.toEqual(deletedArticle);
    expect(calls).toEqual(["votes", "content"]);
    expect(transaction.content_vote.deleteMany).toHaveBeenCalledWith({
      where: { content_id: "article-1" },
    });
    expect(transaction.content.delete).toHaveBeenCalledWith({
      where: { id: "article-1" },
    });
  });

  test("deletes votes before deleting several articles", async () => {
    const calls: string[] = [];
    const transaction = {
      content_vote: {
        deleteMany: jest.fn(async () => {
          calls.push("votes");
          return { count: 3 };
        }),
      },
      content: {
        delete: jest.fn(),
        deleteMany: jest.fn(async () => {
          calls.push("content");
          return { count: 2 };
        }),
      },
    };

    await expect(
      deleteManyContentWithRelations(transaction as never, ["a", "b"])
    ).resolves.toEqual(["a", "b"]);
    expect(calls).toEqual(["votes", "content"]);
    expect(transaction.content_vote.deleteMany).toHaveBeenCalledWith({
      where: { content_id: { in: ["a", "b"] } },
    });
    expect(transaction.content.deleteMany).toHaveBeenCalledWith({
      where: { id: { in: ["a", "b"] } },
    });
  });
});
