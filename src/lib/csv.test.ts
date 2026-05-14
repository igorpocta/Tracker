import { describe, expect, it } from "vitest";

import { buildCsv, csvEscape } from "./csv";

describe("csv", () => {
  it("escapes plain values without quotes", () => {
    expect(csvEscape("hello")).toBe("hello");
    expect(csvEscape(42)).toBe("42");
    expect(csvEscape(null)).toBe("");
    expect(csvEscape(undefined)).toBe("");
  });

  it("quotes values containing commas/newlines/quotes", () => {
    expect(csvEscape("a, b")).toBe('"a, b"');
    expect(csvEscape("line\nbreak")).toBe('"line\nbreak"');
    expect(csvEscape('quote "in" string')).toBe('"quote ""in"" string"');
  });

  it("buildCsv emits header + rows separated by CRLF", () => {
    const out = buildCsv(
      ["a", "b"],
      [
        [1, 2],
        ["x,y", 'with"quote'],
      ],
    );
    const lines = out.split("\r\n");
    // header + 2 rows + trailing newline produces an empty 4th string.
    expect(lines).toHaveLength(4);
    expect(lines[0]).toBe("a,b");
    expect(lines[1]).toBe("1,2");
    expect(lines[2]).toBe('"x,y","with""quote"');
    expect(lines[3]).toBe("");
  });
});
