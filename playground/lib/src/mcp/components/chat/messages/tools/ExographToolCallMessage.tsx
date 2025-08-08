// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { Editor } from "@monaco-editor/react";
import { ExographIcon } from "../../ExographIcon";

interface ExographToolCallMessageProps {
  toolName: string;
  args: any;
  result: any;
  isExpanded: boolean;
  onToggleExpanded: () => void;
}

// Extract and format the GraphQL response from Exograph tool results
function formatExographResult(result: any): string {
  if (result?.content && Array.isArray(result.content)) {
    const responses = result.content
      .map((item: any) => {
        if (item?.text) {
          try {
            const parsed = JSON.parse(item.text);
            if (parsed?.response) {
              // Parse the nested response string to get the GraphQL response
              try {
                const graphqlResponse = JSON.parse(parsed.response);
                // Return an object with the query name as key
                return { [parsed.name]: graphqlResponse };
              } catch {
                return parsed.response;
              }
            }
            return parsed;
          } catch {
            return item.text;
          }
        }
        return null;
      })
      .filter(Boolean);

    return JSON.stringify(responses, null, 2);
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
  // Use custom layout for better width control
  return (
    <div
      className="mb-6"
      role="article"
      aria-label="Tool call message"
      style={{ 
        display: 'flex',
        justifyContent: 'flex-start',
      }}
    >
      <div 
        style={{ 
          display: 'flex',
          flexDirection: 'row',
          width: isExpanded ? 'min(90%, 1024px)' : 'min(80%, 800px)',
        }}
      >
        <div
          className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium mr-3 bg-gray-300 dark:bg-gray-600"
          aria-hidden="true"
        >
          <ExographIcon className="w-5 h-5" />
        </div>
        <div 
          className="px-4 py-3 rounded-2xl bg-green-100 dark:bg-green-800 text-green-900 dark:text-green-100 rounded-bl-md"
          style={{ flex: '1 1 0%' }}
        >
          <button
            onClick={onToggleExpanded}
            className="flex items-center gap-2 text-green-700 dark:text-green-300 font-medium text-sm w-full text-left mb-2"
          >
            {isExpanded ? (
              <ChevronDown className="w-4 h-4" />
            ) : (
              <ChevronRight className="w-4 h-4" />
            )}
            Exograph MCP: {toolName}
          </button>
          {isExpanded && (args || result) && (
            <div className="space-y-3">
              {args?.query && (
                <div>
                  <div className="text-xs font-semibold text-green-700 dark:text-green-300 mb-1">
                    Query:
                  </div>
                  <div className="border border-green-200 dark:border-green-800 rounded">
                    <Editor
                      height="150px"
                      language="graphql"
                      value={args.query}
                      options={{
                        readOnly: true,
                        minimap: { enabled: false },
                        scrollBeyondLastLine: false,
                        fontSize: 12,
                        wordWrap: "on",
                        lineNumbers: "off",
                      }}
                    />
                  </div>
                </div>
              )}
              {result && (
                <div>
                  <div className="text-xs font-semibold text-green-700 dark:text-green-300 mb-1">
                    Result:
                  </div>
                  <div className="border border-green-200 dark:border-green-800 rounded">
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