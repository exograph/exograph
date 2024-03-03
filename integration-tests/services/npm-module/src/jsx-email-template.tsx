import { Text, Html } from "npm:jsx-email";
import * as React from "npm:react";

export default function Email(name: string) {
  return (
    <Html>
      <Text>{name}</Text>
    </Html>
  );
}
