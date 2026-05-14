import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ErrorBoundary } from "./ErrorBoundary";

function Boom({ message = "boom" }: { message?: string }): ReactElement {
  throw new Error(message);
}

function Ok(): ReactElement {
  return <div data-testid="ok">all good</div>;
}

describe("ErrorBoundary", () => {
  // React logs to console.error when a boundary catches; silence it.
  const origConsole = console.error;
  afterEach(() => {
    console.error = origConsole;
  });

  it("renders children when no error is thrown", () => {
    render(
      <ErrorBoundary>
        <Ok />
      </ErrorBoundary>,
    );
    expect(screen.getByTestId("ok")).toBeInTheDocument();
  });

  it("renders the fallback UI when a child throws", () => {
    console.error = vi.fn();
    render(
      <ErrorBoundary>
        <Boom message="kaboom!" />
      </ErrorBoundary>,
    );
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText(/Something went wrong/i)).toBeInTheDocument();
    expect(screen.getByText(/kaboom!/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /reload/i })).toBeInTheDocument();
  });

  it("uses the custom fallback when provided", () => {
    console.error = vi.fn();
    render(
      <ErrorBoundary
        fallback={(error) => <div data-testid="custom">caught: {error.message}</div>}
      >
        <Boom message="custom boom" />
      </ErrorBoundary>,
    );
    expect(screen.getByTestId("custom")).toHaveTextContent("caught: custom boom");
  });

  it("invokes the reset callback when the fallback's reset button is clicked", () => {
    console.error = vi.fn();
    const reset = vi.fn();
    const fallback = vi.fn((_error: Error, fbReset: () => void) => {
      reset.mockImplementation(fbReset);
      return (
        <button type="button" onClick={reset}>
          reset me
        </button>
      );
    });
    render(
      <ErrorBoundary fallback={fallback}>
        <Boom message="will be reset" />
      </ErrorBoundary>,
    );
    expect(fallback).toHaveBeenCalled();
    fireEvent.click(screen.getByText("reset me"));
    expect(reset).toHaveBeenCalled();
  });
});
