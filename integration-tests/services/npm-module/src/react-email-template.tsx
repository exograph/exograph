import { Text, Html } from "npm:@react-email/components@1.0.10";
import * as React from "npm:react";

export default function Email({ name }: { name: string }) {
  return (
    <Html>
      <Text>{name}</Text>
    </Html>
  );
}
