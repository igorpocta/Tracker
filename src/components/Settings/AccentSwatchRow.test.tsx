import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ACCENTS } from "../../lib/accent";
import { AccentSwatchRow } from "./AccentSwatchRow";

describe("AccentSwatchRow", () => {
  it("renders one swatch per accent in the palette", () => {
    render(<AccentSwatchRow value="blue" onChange={() => {}} />);
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(ACCENTS.length);
  });

  it("marks only the active swatch as checked", () => {
    render(<AccentSwatchRow value="violet" onChange={() => {}} />);
    const violet = screen.getByLabelText("Violet");
    const blue = screen.getByLabelText("Blue");
    expect(violet).toHaveAttribute("aria-checked", "true");
    expect(blue).toHaveAttribute("aria-checked", "false");
  });

  it("calls onChange with the clicked accent id", async () => {
    const onChange = vi.fn();
    render(<AccentSwatchRow value="blue" onChange={onChange} />);
    await userEvent.click(screen.getByLabelText("Pink"));
    expect(onChange).toHaveBeenCalledWith("pink");
  });
});
