// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Wrench, Loader2 } from "lucide-react";
import { ToolErrorDisplay } from "./ToolErrorDisplay";
import { ToolCallCard, useToolCallStatus, toolCallColors } from "./ToolCallCard";
import { extractToolError } from "../../../types/ToolError";
import type { MessagePartStatus, ToolCallMessagePartStatus } from "@assistant-ui/react";

interface DefaultToolRenderProps {
  toolName: string;
  args: Record<string, unknown>;
  result?: unknown;
  status: MessagePartStatus | ToolCallMessagePartStatus;
}

const DefaultToolRender = memo(function DefaultToolRender({
  toolName, args, result, status,
}: DefaultToolRenderProps) {
  const { isRunning, isError } = useToolCallStatus(status);
  const colors = toolCallColors(isError);
  const stateLabel = isRunning ? " (Executing...)" : isError ? " (Failed)" : "";

  return (
    <ToolCallCard
      title={`Tool: ${toolName}`}
      isError={isError}
      avatar={isRunning ? <Loader2 className="w-4 h-4 animate-spin" /> : <Wrench className="w-4 h-4" />}
      stateLabel={stateLabel}
    >
      <div className="space-y-3">
        {args && (
          <div>
            <div className={`text-xs font-semibold ${colors.label} mb-1`}>Arguments:</div>
            <div className={`text-xs ${colors.codeText} font-mono ${colors.codeBg} p-2 rounded opacity-75`}>
              {JSON.stringify(args, null, 2)}
            </div>
          </div>
        )}
        {isError && <ToolErrorDisplay error={extractToolError(result)} toolName={toolName} />}
        {!isError && result != null && (
          <div>
            <div className={`text-xs font-semibold ${colors.label} mb-1`}>Result:</div>
            <div className={`text-xs ${colors.codeText} font-mono ${colors.codeBg} p-2 rounded opacity-75`}>
              {JSON.stringify(result, null, 2)}
            </div>
          </div>
        )}
      </div>
    </ToolCallCard>
  );
});

export const DefaultToolUI = makeAssistantToolUI<Record<string, unknown>, unknown>({
  toolName: "*",
  render: ({ toolName, args, result, status }) => (
    <DefaultToolRender toolName={toolName} args={args} result={result} status={status} />
  ),
});
