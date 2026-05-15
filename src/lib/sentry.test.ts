/**
 * Unit tests for the frontend Sentry scrubbing helpers. These are pure
 * functions with no SDK side-effects so we can exercise them directly
 * without booting Sentry itself.
 */
import { describe, expect, it } from "vitest";

import { looksLikeToken, maskEmail, scrubObject } from "./sentry";

describe("maskEmail", () => {
  it("masks all but the first letter of the local part", () => {
    expect(maskEmail("igor.pocta@example.com")).toBe("i***@example.com");
  });

  it("falls back to *** when the local part is a single character", () => {
    expect(maskEmail("i@x.cz")).toBe("***@x.cz");
  });

  it("redacts strings with no '@' altogether", () => {
    expect(maskEmail("notanemail")).toBe("[redacted]");
  });

  it("preserves the domain casing", () => {
    expect(maskEmail("Alice@Example.COM")).toBe("A***@Example.COM");
  });
});

describe("looksLikeToken", () => {
  it("flags long base64/hex-ish strings", () => {
    expect(looksLikeToken("ATATT3xFfGN0abcdef1234567890XYZ")).toBe(true);
    expect(
      looksLikeToken("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.signature"),
    ).toBe(true);
  });

  it("does not flag short strings", () => {
    expect(looksLikeToken("hi")).toBe(false);
    expect(looksLikeToken("short")).toBe(false);
  });

  it("does not flag strings with spaces", () => {
    expect(looksLikeToken("this is a sentence with many words 12345")).toBe(
      false,
    );
  });

  it("does not flag very long strings (probably content, not a token)", () => {
    expect(looksLikeToken("a".repeat(201))).toBe(false);
  });
});

describe("scrubObject", () => {
  it("redacts known-sensitive keys", () => {
    const obj: Record<string, unknown> = {
      api_key: "sk_live_abcdef",
      apiKey: "zzz",
      Authorization: "Bearer xyz",
      PASSWORD: "hunter2",
      sessionCookie: "c",
      bearer_token: "abc",
      safe: "hello",
    };
    scrubObject(obj);
    expect(obj.api_key).toBe("[redacted]");
    expect(obj.apiKey).toBe("[redacted]");
    expect(obj.Authorization).toBe("[redacted]");
    expect(obj.PASSWORD).toBe("[redacted]");
    expect(obj.sessionCookie).toBe("[redacted]");
    expect(obj.bearer_token).toBe("[redacted]");
    // Untouched.
    expect(obj.safe).toBe("hello");
  });

  it("redacts long alphanumeric values defensively", () => {
    const obj: Record<string, unknown> = {
      weird: "ATATT3xFfGN0abcdef1234567890XYZ-LongTokenLookingThing.suffix==",
    };
    scrubObject(obj);
    expect(obj.weird).toBe("[redacted-token]");
  });

  it("recurses into nested objects", () => {
    const obj = {
      outer: {
        nested: {
          api_key: "deep",
          ok: "fine",
        },
      },
    };
    scrubObject(obj);
    expect(obj.outer.nested.api_key).toBe("[redacted]");
    expect(obj.outer.nested.ok).toBe("fine");
  });

  it("leaves null / undefined / primitives alone", () => {
    // None of these should throw.
    scrubObject(null);
    scrubObject(undefined);
    scrubObject(42 as unknown);
    scrubObject("string" as unknown);
  });

  it("handles arrays of objects gracefully", () => {
    const obj: { items: Array<Record<string, unknown>> } = {
      items: [{ token: "x" }, { ok: "y" }],
    };
    scrubObject(obj);
    expect(obj.items[0].token).toBe("[redacted]");
    expect(obj.items[1].ok).toBe("y");
  });
});
