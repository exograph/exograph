// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";
import { Wrench } from "lucide-react";
import { ToolMessageContainer } from "./ToolMessageContainer";

export interface GenericToolCallMessageProps {
  toolName: string;
  args: any;
  result: any;
  isExpanded: boolean;
  onToggleExpanded: () => void;
}

// Generic tool call message component for non-Exograph tools
export const GenericToolCallMessage = memo(function GenericToolCallMessage({
  toolName,
  args,
  result,
  isExpanded,
  onToggleExpanded,
}: GenericToolCallMessageProps) {
  return (
    <ToolMessageContainer
      icon={<Wrench className="w-4 h-4" />}
      iconBgColor="bg-indigo-500 dark:bg-indigo-600 text-white"
      messageBgColor="bg-indigo-100 dark:bg-indigo-800"
      textColor="text-indigo-900 dark:text-indigo-100"
      buttonTextColor="text-indigo-700 dark:text-indigo-300"
      toolLabel={`Tool: ${toolName}`}
      isExpanded={isExpanded}
      onToggleExpanded={onToggleExpanded}
    >
      {(args || result) && (
        <>
          {args && (
            <div>
              <div className="text-xs font-semibold text-indigo-700 dark:text-indigo-300 mb-1">
                Arguments:
              </div>
              <div className="text-xs text-indigo-600 dark:text-indigo-400 font-mono bg-indigo-100 dark:bg-indigo-900/50 p-2 rounded">
                {JSON.stringify(args, null, 2)}
              </div>
            </div>
          )}
          {result && (
            <div>
              <div className="text-xs font-semibold text-indigo-700 dark:text-indigo-300 mb-1">
                Result:
              </div>
              <div className="text-xs text-indigo-600 dark:text-indigo-400 font-mono bg-indigo-100 dark:bg-indigo-900/50 p-2 rounded">
                {JSON.stringify(result, null, 2)}
              </div>
            </div>
          )}
        </>
      )}
    </ToolMessageContainer>
  );
});