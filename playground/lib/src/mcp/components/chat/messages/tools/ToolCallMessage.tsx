// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo, useState } from "react";
import {
  BaseMessage,
  ToolMessage,
  isToolMessage,
} from "../../../../providers/ChatMessage";
import { ExographToolCallMessage } from "./ExographToolCallMessage";
import { GenericToolCallMessage } from "./GenericToolCallMessage";

interface ToolCallMessageProps {
  message: BaseMessage;
}

export const ToolCallMessage = memo(function ToolCallMessage({
  message,
}: ToolCallMessageProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  // Ensure this is a tool message
  if (!isToolMessage(message)) {
    return null;
  }

  const toolMessage = message as ToolMessage;
  const toolName = toolMessage.toolName;
  const result = toolMessage.result;
  const args = toolMessage.args;

  // Special handling for Exograph tools
  return toolName.startsWith("execute_query") ? (
    <ExographToolCallMessage
      toolName={toolName}
      args={args}
      result={result}
      isExpanded={isExpanded}
      onToggleExpanded={() => setIsExpanded(!isExpanded)}
    />
  ) : (
    // Generic tool call message for non-Exograph tools
    <GenericToolCallMessage
      toolName={toolName}
      args={args}
      result={result}
      isExpanded={isExpanded}
      onToggleExpanded={() => setIsExpanded(!isExpanded)}
    />
  );
});
