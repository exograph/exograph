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
import { ShieldX, CheckCircle, XCircle, Loader2 } from "lucide-react";
import { Editor } from "@monaco-editor/react";
import { ExographIcon } from "../ExographIcon";
import { ToolErrorDisplay } from "./ToolErrorDisplay";
import { ToolCallCard, useToolCallStatus, toolCallColors } from "./ToolCallCard";
import { useTheme } from "../../../../util/theme";
import { isAuthToolError, extractToolError } from "../../../types/ToolError";
import type { MessagePartStatus, ToolCallMessagePartStatus } from "@assistant-ui/react";

function getQueryFromArgs(args: Record<string, unknown> | null | undefined): string | null {
  if (!args || !args.query || typeof args.query !== "string") return null;
  return args.query;
}

function formatExographResult(result: unknown): string {
  if (result != null && typeof result === 'object') {
    const obj = result as Record<string, unknown>;
    const content = obj.value || obj.content;
    if (content && Array.isArray(content)) {
      const responses: Record<string, unknown> = {};
      for (const item of content) {
        if (item != null && typeof item === 'object') {
          const entry = item as Record<string, unknown>;
          if (typeof entry.text === 'string') {
            try {
              const parsed = JSON.parse(entry.text);
              if (parsed?.name && parsed?.response) {
                try { responses[parsed.name] = JSON.parse(parsed.response); }
                catch { responses[parsed.name] = parsed.response; }
              }
            } catch { /* skip */ }
          }
        }
      }
      if (Object.keys(responses).length > 0) return JSON.stringify(responses, null, 2);
    }
  }
  return JSON.stringify(result, null, 2);
}

const READONLY_EDITOR_OPTIONS = {
  readOnly: true,
  minimap: { enabled: false },
  scrollBeyondLastLine: false,
  fontSize: 12,
  wordWrap: "on" as const,
  lineNumbers: "off" as const,
};

interface ExographToolRenderProps {
  toolName: string;
  args: Record<string, unknown>;
  result?: unknown;
  status: MessagePartStatus | ToolCallMessagePartStatus;
}

const ExographToolRender = memo(function ExographToolRender({
  toolName, args, result, status,
}: ExographToolRenderProps) {
  const editorTheme = useTheme() === "dark" ? "vs-dark" : "light";
  const { isRunning, isComplete, isError } = useToolCallStatus(status);
  const isAuth = isAuthToolError(result);
  const hasResultError = result != null && typeof result === 'object' && '_toolError' in (result as Record<string, unknown>);
  const hasError = isError || isAuth || hasResultError;
  const colors = toolCallColors(hasError);
  const queryString = getQueryFromArgs(args);

  const statusIcon = isRunning ? <Loader2 className="w-4 h-4 animate-spin" />
    : isAuth ? <ShieldX className="w-4 h-4" />
    : hasError ? <XCircle className="w-4 h-4" />
    : isComplete ? <CheckCircle className="w-4 h-4" />
    : null;

  const stateLabel = isRunning ? " (Executing...)"
    : isAuth ? " (Access Denied)"
    : hasError ? " (Failed)"
    : "";

  return (
    <ToolCallCard
      title={`Exograph MCP: ${toolName}`}
      isError={hasError}
      avatar={<ExographIcon className="w-5 h-5" />}
      statusIcon={statusIcon}
      stateLabel={stateLabel}
    >
      <div className="space-y-3">
        {queryString && (
          <div>
            <div className={`text-xs font-semibold ${colors.label} mb-1`}>Query:</div>
            <div className={`border ${colors.border} rounded`}>
              <Editor height="150px" language="graphql" value={queryString} options={READONLY_EDITOR_OPTIONS} theme={editorTheme} />
            </div>
          </div>
        )}
        {hasError && <ToolErrorDisplay error={extractToolError(result)} toolName={toolName} />}
        {result != null && !hasError && (
          <div>
            <div className="text-xs font-semibold text-green-700 dark:text-green-300 mb-1">Result:</div>
            <div className="border border-green-200 dark:border-green-800 rounded bg-green-50 dark:bg-green-900/20">
              <Editor height="200px" language="json" value={formatExographResult(result)} options={READONLY_EDITOR_OPTIONS} theme={editorTheme} />
            </div>
          </div>
        )}
      </div>
    </ToolCallCard>
  );
});

const renderExographTool = ({ toolName, args, result, status }: ExographToolRenderProps) => (
  <ExographToolRender toolName={toolName} args={args} result={result} status={status} />
);

export const ExographToolUI = makeAssistantToolUI<Record<string, unknown>, unknown>({
  toolName: "execute_query",
  render: renderExographTool,
});

export const ExographWithVarsToolUI = makeAssistantToolUI<Record<string, unknown>, unknown>({
  toolName: "execute_query_with_variables",
  render: renderExographTool,
});
