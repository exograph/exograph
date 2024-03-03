import { Text, Html } from "npm:@react-email/components";
import * as React from "npm:react";

export default function Email(name: string) {
  return (
    <Html>
      <Text>{name}</Text>
    </Html>
  );
}
