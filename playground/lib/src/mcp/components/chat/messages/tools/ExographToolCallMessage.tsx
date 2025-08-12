// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";
import { ChevronDown, ChevronRight, ShieldX } from "lucide-react";
import { Editor } from "@monaco-editor/react";
import { ExographIcon } from "../../ExographIcon";
import { useTheme } from "../../../../../util/theme";

/**
 * Tool call result from ChatAPI - can be a successful MCP result,
 * an error object, or a mixed object with various properties.
 * This is intentionally flexible since the result comes from
 * AI SDK tool execution which can have various formats.
 */
type ToolCallResult =
  | {
      error?: any;
      status?: number;
      content?: any;
      [key: string]: any;
    }
  | null
  | undefined;

/**
 * Tool call arguments from MCP protocol.
 * For Exograph tools, this typically contains a 'query' field with the GraphQL query.
 */
type ToolCallArgs = Record<string, unknown>;

// Helper to safely extract query string from args
function getQueryFromArgs(
  args: ToolCallArgs | null | undefined
): string | null {
  if (!args || !args.query || typeof args.query !== "string") {
    return null;
  }
  return args.query;
}

interface ExographToolCallMessageProps {
  toolName: string;
  args: ToolCallArgs;
  result: ToolCallResult;
  isExpanded: boolean;
  onToggleExpanded: () => void;
}

// Extract error information from the result
function extractError(result: ToolCallResult): string | null {
  if (!result) return null;

  // Check for direct error property first (from AI SDK)
  if (result.error) {
    return typeof result.error === "string"
      ? result.error
      : JSON.stringify(result.error, null, 2);
  }

  // Check content array for errors
  if (result?.content && Array.isArray(result.content)) {
    for (const item of result.content) {
      if (item?.text) {
        try {
          const parsed = JSON.parse(item.text);
          if (parsed.error)
            return typeof parsed.error === "string"
              ? parsed.error
              : JSON.stringify(parsed.error, null, 2);
        } catch {
          // Don't treat parse failures as errors - they're just data
        }
      }
    }
  }

  return null;
}

// Extract HTTP status code from error result
function extractHttpStatus(result: ToolCallResult): number | null {
  // Check if result has a direct status property
  if (result?.status && typeof result.status === "number") {
    return result.status;
  }

  // Check if there's an error with status
  if (result?.error?.status && typeof result.error.status === "number") {
    return result.error.status;
  }

  return null;
}

// Check if the status code indicates authentication/authorization error
function isAuthError(result: ToolCallResult): boolean {
  const status = extractHttpStatus(result);
  return status === 401 || status === 403;
}

// Extract and format the GraphQL response from Exograph tool results
function formatExographResult(result: ToolCallResult): string {
  // Try to extract from result.output.content first (AI SDK tool result format)
  const content = result?.output?.content || result?.content;
  
  if (content && Array.isArray(content)) {
    const responses: Record<string, any> = {};
    
    for (const item of content) {
      if (item?.text) {
        try {
          const parsed = JSON.parse(item.text);
          if (parsed?.name && parsed?.response) {
            // Parse the response string to get the actual GraphQL response
            try {
              responses[parsed.name] = JSON.parse(parsed.response);
            } catch {
              responses[parsed.name] = parsed.response;
            }
          }
        } catch {
          // Skip items that can't be parsed
        }
      }
    }

    if (Object.keys(responses).length > 0) {
      return JSON.stringify(responses, null, 2);
    }
  }
  
  return JSON.stringify(result, null, 2);
}

export const ExographToolCallMessage = memo(function ExographToolCallMessage({
  toolName,
  args,
  result,
  isExpanded,
  onToggleExpanded,
}: ExographToolCallMessageProps) {
  const theme = useTheme();
  const editorTheme = theme === "dark" ? "vs-dark" : "light";
  const errorText = extractError(result);
  const isAuth = isAuthError(result);
  const hasError = errorText !== null || isAuth;
  const queryString = getQueryFromArgs(args);

  // Main bubble background: green for success, red for errors, avatar stays gray
  const bgColor = hasError
    ? "bg-red-100 dark:bg-red-900/30"
    : "bg-green-100 dark:bg-green-800";
  const textColor = hasError
    ? "text-red-900 dark:text-red-100"
    : "text-green-900 dark:text-green-100";
  const borderColor = hasError
    ? "border-red-200 dark:border-red-800"
    : "border-green-200 dark:border-green-800";
  const labelColor = hasError
    ? "text-red-700 dark:text-red-300"
    : "text-green-700 dark:text-green-300";
  const buttonColor = hasError
    ? "text-red-700 dark:text-red-300"
    : "text-green-700 dark:text-green-300";
  const avatarBg = "bg-gray-300 dark:bg-gray-600";
  const errorEditorBg = hasError ? "bg-red-50 dark:bg-red-900/20" : "";
  const successEditorBg = !hasError ? "bg-green-50 dark:bg-green-900/20" : "";

  return (
    <div
      className="mb-6 flex justify-start"
      role="article"
      aria-label="Tool call message"
    >
      <div
        className={`flex flex-row ${isExpanded ? "w-[min(90%,_1024px)]" : "w-[min(80%,_800px)]"}`}
      >
        <div
          className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium mr-3 ${avatarBg} text-white`}
          aria-hidden="true"
        >
          <ExographIcon className="w-5 h-5" />
        </div>
        <div
          className={`flex-1 px-4 py-3 rounded-2xl ${bgColor} ${textColor} rounded-bl-md`}
        >
          <button
            onClick={onToggleExpanded}
            className={`flex items-center gap-2 ${buttonColor} font-medium text-sm w-full text-left mb-2`}
          >
            {isExpanded ? (
              <ChevronDown className="w-4 h-4" />
            ) : (
              <ChevronRight className="w-4 h-4" />
            )}
            {isAuth && <ShieldX className="w-4 h-4" />}
            Exograph MCP: {toolName}
            {hasError && !isAuth && " (Error)"}
            {isAuth && " (Access Denied)"}
          </button>
          {isExpanded && (args || result) && (
            <div className="space-y-3">
              {queryString && (
                <div>
                  <div className={`text-xs font-semibold ${labelColor} mb-1`}>
                    Query:
                  </div>
                  <div className={`border ${borderColor} rounded`}>
                    <Editor
                      height="150px"
                      language="graphql"
                      value={queryString}
                      options={{
                        readOnly: true,
                        minimap: { enabled: false },
                        scrollBeyondLastLine: false,
                        fontSize: 12,
                        wordWrap: "on",
                        lineNumbers: "off",
                      }}
                      theme={editorTheme}
                    />
                  </div>
                </div>
              )}
              {hasError && errorText && !isAuth && (
                <div>
                  <div className={`text-xs font-semibold ${labelColor} mb-1`}>
                    Error:
                  </div>
                  <div
                    className={`border ${borderColor} rounded ${errorEditorBg}`}
                  >
                    <Editor
                      height="150px"
                      language="json"
                      value={errorText}
                      options={{
                        readOnly: true,
                        minimap: { enabled: false },
                        scrollBeyondLastLine: false,
                        fontSize: 12,
                        wordWrap: "on",
                        lineNumbers: "off",
                      }}
                      theme={editorTheme}
                    />
                  </div>
                </div>
              )}
              {result && !hasError && (
                <div>
                  <div className={`text-xs font-semibold ${labelColor} mb-1`}>
                    Result:
                  </div>
                  <div className={`border ${borderColor} rounded ${successEditorBg}`}>
                    <Editor
                      height="200px"
                      language="json"
                      value={formatExographResult(result)}
                      options={{
                        readOnly: true,
                        minimap: { enabled: false },
                        scrollBeyondLastLine: false,
                        fontSize: 12,
                        wordWrap: "on",
                        lineNumbers: "off",
                      }}
                      theme={editorTheme}
                    />
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
