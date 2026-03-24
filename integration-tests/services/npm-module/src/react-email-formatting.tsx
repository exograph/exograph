import { render } from "npm:@react-email/render";
import * as React from "npm:react";
import Email from "./react-email-template.tsx";

export async function formatReactEmail(name: string): Promise<string> {
  return render(<Email name={name} />);
}
