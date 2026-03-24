import { Text, Html } from "npm:jsx-email@2.8.4";
import * as React from "npm:react";

export default function Email({ name }: { name: string }) {
  return (
    <Html>
      <Text>{name}</Text>
    </Html>
  );
}
