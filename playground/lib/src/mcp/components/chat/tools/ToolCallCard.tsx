// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useState, type ReactNode } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { isErrorStatus } from "../../../types/ToolError";

/** Derive common status flags from a tool call's MessagePartStatus. */
export function useToolCallStatus(status: { type: string }) {
  const isRunning = status.type === "running";
  const isComplete = status.type === "complete";
  const isError = isErrorStatus(status);
  return { isRunning, isComplete, isError };
}

/** Color classes keyed on error state. */
export const toolCallColors = (isError: boolean) => ({
  label: isError ? "text-red-700 dark:text-red-300" : "text-green-700 dark:text-green-300",
  border: isError ? "border-red-200 dark:border-red-800" : "border-green-200 dark:border-green-800",
  codeBg: isError ? "bg-red-100 dark:bg-red-800" : "bg-green-100 dark:bg-green-800",
  codeText: isError ? "text-red-900 dark:text-red-100" : "text-green-900 dark:text-green-100",
});

export interface ToolCallCardProps {
  title: string;
  isError: boolean;
  avatar: ReactNode;
  statusIcon?: ReactNode;
  stateLabel?: string;
  expanded?: boolean;
  children?: ReactNode;
}

export function ToolCallCard({
  title,
  isError,
  avatar,
  statusIcon,
  stateLabel,
  expanded: defaultExpanded,
  children,
}: ToolCallCardProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded ?? false);

  const cardBg = isError
    ? "bg-red-100 dark:bg-red-900/30 text-red-900 dark:text-red-100"
    : "bg-green-100 dark:bg-green-800 text-green-900 dark:text-green-100";

  const buttonColor = isError
    ? "text-red-700 dark:text-red-300"
    : "text-green-700 dark:text-green-300";

  return (
    <div className="mb-6 flex justify-start" role="article" aria-label="Tool call message">
      <div className={`flex flex-row ${isExpanded ? "w-[min(90%,1024px)]" : "w-[min(80%,800px)]"}`}>
        <div className="mr-3">
          <div className="shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium bg-gray-300 dark:bg-gray-600 text-white">
            {avatar}
          </div>
        </div>
        <div className={`flex-1 px-4 py-3 rounded-2xl rounded-bl-md ${cardBg}`}>
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className={`flex items-center gap-2 ${buttonColor} font-medium text-sm w-full text-left mb-2`}
          >
            {isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
            {statusIcon}
            {title}
            {stateLabel}
          </button>
          {isExpanded && children}
        </div>
      </div>
    </div>
  );
}
