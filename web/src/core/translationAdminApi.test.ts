import handler from "../pages/api/admin/translation-operations";
import { requireSession } from "./serverSession";

jest.mock("./serverSession", () => ({ requireSession: jest.fn() }));
jest.mock("../../../config-runtime", () => ({
  getWibbleConfig: () => ({ app: { site_url: "https://wibble.fbmac.net" } }),
}));

const snapshot = jest.fn();
const updateHourlyLimit = jest.fn();
const resetBudget = jest.fn();
const removeJob = jest.fn();
jest.mock("./TranslationAdminService", () => ({
  TranslationAdminService: jest.fn().mockImplementation(() => ({
    snapshot: (...args: any[]) => snapshot(...args),
    updateHourlyLimit: (...args: any[]) => updateHourlyLimit(...args),
    resetBudget: (...args: any[]) => resetBudget(...args),
    removeJob: (...args: any[]) => removeJob(...args),
  })),
}));

const response = () => {
  const res: any = {};
  res.status = jest.fn(() => res);
  res.json = jest.fn(() => res);
  return res;
};

const sameOriginHeaders = {
  origin: "https://wibble.fbmac.net",
  host: "wibble.fbmac.net",
  "content-type": "application/json",
};

describe("translation operations admin API", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    (requireSession as jest.Mock).mockResolvedValue(true);
    snapshot.mockResolvedValue({ hourlyLimit: 10, used: 4, remaining: 6, jobs: [] });
    updateHourlyLimit.mockResolvedValue({ hourlyLimit: 6 });
    resetBudget.mockResolvedValue({ resetAttempts: 4 });
    removeJob.mockResolvedValue({ removed: true });
  });

  test("is admin-only and returns the ordered queue snapshot", async () => {
    (requireSession as jest.Mock).mockResolvedValueOnce(false);
    const denied = response();
    await handler({ method: "GET" } as any, denied);
    expect(snapshot).not.toHaveBeenCalled();

    const allowed = response();
    await handler({ method: "GET" } as any, allowed);
    expect(allowed.status).toHaveBeenCalledWith(200);
    expect(allowed.json).toHaveBeenCalledWith(expect.objectContaining({ hourlyLimit: 10, jobs: [] }));
  });

  test("updates a bounded integer hourly limit", async () => {
    const invalid = response();
    await handler({ method: "PATCH", headers: sameOriginHeaders, body: { hourlyLimit: 2.5 } } as any, invalid);
    expect(invalid.status).toHaveBeenCalledWith(400);
    expect(updateHourlyLimit).not.toHaveBeenCalled();

    const valid = response();
    await handler({ method: "PATCH", headers: sameOriginHeaders, body: { hourlyLimit: 6 } } as any, valid);
    expect(updateHourlyLimit).toHaveBeenCalledWith(6);
    expect(valid.status).toHaveBeenCalledWith(200);
  });

  test("resets only the automatic rolling budget", async () => {
    const res = response();
    await handler({ method: "POST", headers: sameOriginHeaders, body: { action: "reset-budget" } } as any, res);
    expect(resetBudget).toHaveBeenCalledTimes(1);
    expect(res.json).toHaveBeenCalledWith({ resetAttempts: 4 });
  });

  test("removes a queued task in one request and protects processing work", async () => {
    const removed = response();
    await handler({ method: "DELETE", headers: sameOriginHeaders, query: { id: "job-1" } } as any, removed);
    expect(removeJob).toHaveBeenCalledWith("job-1");
    expect(removed.status).toHaveBeenCalledWith(200);

    removeJob.mockResolvedValueOnce({ removed: false, reason: "processing" });
    const active = response();
    await handler({ method: "DELETE", headers: sameOriginHeaders, query: { id: "job-active" } } as any, active);
    expect(active.status).toHaveBeenCalledWith(409);
  });

  test("rejects cross-origin and form-encoded mutations", async () => {
    const crossOrigin = response();
    await handler({
      method: "POST",
      headers: { ...sameOriginHeaders, origin: "https://attacker.example" },
      body: { action: "reset-budget" },
    } as any, crossOrigin);
    expect(crossOrigin.status).toHaveBeenCalledWith(403);

    const formPost = response();
    await handler({
      method: "POST",
      headers: { ...sameOriginHeaders, "content-type": "application/x-www-form-urlencoded" },
      body: { action: "reset-budget" },
    } as any, formPost);
    expect(formPost.status).toHaveBeenCalledWith(415);
    expect(resetBudget).not.toHaveBeenCalled();
  });

  test("does not trust spoofed forwarded headers for mutation origin", async () => {
    const spoofed = response();
    await handler({
      method: "POST",
      headers: {
        ...sameOriginHeaders,
        origin: "https://attacker.example",
        "x-forwarded-host": "attacker.example",
        "x-forwarded-proto": "https",
      },
      body: { action: "reset-budget" },
    } as any, spoofed);
    expect(spoofed.status).toHaveBeenCalledWith(403);
    expect(resetBudget).not.toHaveBeenCalled();
  });
});
