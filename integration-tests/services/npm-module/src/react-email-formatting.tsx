import { render } from "npm:@react-email/render";
import * as React from "npm:react";
import Email from "./react-email-template.tsx";

export function formatReactEmail(name: string): string {
  render(<Email name={name} />);
}
