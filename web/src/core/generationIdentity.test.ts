import { generationSafetyIdentifier } from "./generationIdentity";

describe("generationSafetyIdentifier", () => {
  test("prefers a normalized email and does not expose it", () => {
    const identifier = generationSafetyIdentifier(
      " User@Example.com ",
      "192.0.2.10"
    );

    expect(identifier).toMatch(/^[a-f0-9]{64}$/);
    expect(identifier).not.toContain("user@example.com");
    expect(identifier).toBe(
      generationSafetyIdentifier("user@example.com", "198.51.100.2")
    );
  });

  test("uses a normalized IP when there is no email", () => {
    expect(generationSafetyIdentifier(null, " 192.0.2.10 ")).toBe(
      generationSafetyIdentifier(null, "192.0.2.10")
    );
    expect(generationSafetyIdentifier(null, "192.0.2.10")).not.toBe(
      generationSafetyIdentifier(null, "192.0.2.11")
    );
  });

  test("omits the identifier when neither value exists", () => {
    expect(generationSafetyIdentifier(null, null)).toBeUndefined();
  });
});
