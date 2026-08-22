import handler from "../pages/api/translations/queue";
import { getServerEmail } from "./serverSession";
import { TranslationQueueService } from "./TranslationQueueService";

const mockEnqueueVisible = jest.fn().mockResolvedValue(2);
jest.mock("./serverSession", () => ({ getServerEmail: jest.fn() }));
jest.mock("./TranslationQueueService", () => ({
  TranslationQueueService: jest.fn().mockImplementation(() => ({
    enqueueVisible: mockEnqueueVisible,
  })),
}));

const response = () => {
  const res: any = {};
  res.status = jest.fn(() => res);
  res.json = jest.fn(() => res);
  return res;
};

describe("translation queue API", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockEnqueueVisible.mockResolvedValue(2);
  });

  test("requires an authenticated user", async () => {
    (getServerEmail as jest.Mock).mockResolvedValue(null);
    const res = response();
    await handler({ method: "POST", body: { language: "pt-BR", slugs: ["one"] } } as any, res);
    expect(res.status).toHaveBeenCalledWith(401);
  });

  test("queues a bounded visible batch", async () => {
    (getServerEmail as jest.Mock).mockResolvedValue("user@example.com");
    const res = response();
    await handler({ method: "POST", body: { language: "pt-BR", slugs: ["one", "two"] } } as any, res);
    expect(TranslationQueueService).toHaveBeenCalledTimes(1);
    expect(res.status).toHaveBeenCalledWith(202);
    expect(res.json).toHaveBeenCalledWith({ queued: 2 });
  });

  test("distinguishes invalid language from an infrastructure failure", async () => {
    (getServerEmail as jest.Mock).mockResolvedValue("user@example.com");
    const invalid = response();
    await handler({ method: "POST", body: { language: "invalid language", slugs: ["one"] } } as any, invalid);
    expect(invalid.status).toHaveBeenCalledWith(400);

    mockEnqueueVisible.mockRejectedValueOnce(new Error("database unavailable"));
    const failed = response();
    await handler({ method: "POST", body: { language: "pt-BR", slugs: ["one"] } } as any, failed);
    expect(failed.status).toHaveBeenCalledWith(500);
  });
});
