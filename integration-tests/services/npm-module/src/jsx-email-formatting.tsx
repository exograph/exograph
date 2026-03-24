import { render } from "npm:jsx-email@2.8.4";
import * as React from "npm:react";
import Email from "./jsx-email-template.tsx";

export function formatJsxEmail(name: string): string {
  return render(<Email name={name} />);
}
