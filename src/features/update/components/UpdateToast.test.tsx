// @vitest-environment jsdom
import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UpdateToast } from "./UpdateToast";

describe("UpdateToast custom-distribution boundary", () => {
  it("renders no update action surface", () => {
    const { container } = render(
      <UpdateToast
        state={{ stage: "disabled" }}
        onUpdate={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );

    expect(container.innerHTML).toBe("");
  });
});
