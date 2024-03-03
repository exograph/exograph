import { render } from "npm:jsx-email";
import * as React from "npm:react";
import Email from "./jsx-email-template.tsx";

export function formatJsxEmail(name: string): string {
  render(<Email name={name} />);
}
