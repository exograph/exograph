// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo, ReactNode } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

export interface ToolMessageContainerProps {
  icon: ReactNode;
  iconBgColor?: string;
  messageBgColor: string;
  textColor: string;
  buttonTextColor: string;
  toolLabel: string;
  isExpanded: boolean;
  onToggleExpanded: () => void;
  children?: ReactNode;
  maxWidth?: string;
}

// Container component for all tool messages
export const ToolMessageContainer = memo(function ToolMessageContainer({
  icon,
  iconBgColor = "bg-gray-300 dark:bg-gray-600",
  messageBgColor,
  textColor,
  buttonTextColor,
  toolLabel,
  isExpanded,
  onToggleExpanded,
  children,
  maxWidth = "max-w-[80%]",
}: ToolMessageContainerProps) {
  return (
    <div
      className="flex mb-6 justify-start"
      role="article"
      aria-label="Tool call message"
    >
      <div className={`flex ${maxWidth} flex-row`}>
        <div
          className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium mr-3 ${iconBgColor}`}
          aria-hidden="true"
        >
          {icon}
        </div>
        <div
          className={`px-4 py-3 rounded-2xl ${messageBgColor} ${textColor} rounded-bl-md`}
        >
          <button
            onClick={onToggleExpanded}
            className={`flex items-center gap-2 ${buttonTextColor} font-medium text-sm w-full text-left mb-2`}
          >
            {isExpanded ? (
              <ChevronDown className="w-4 h-4" />
            ) : (
              <ChevronRight className="w-4 h-4" />
            )}
            {toolLabel}
          </button>
          {isExpanded && children && (
            <div className="space-y-3">{children}</div>
          )}
        </div>
      </div>
    </div>
  );
});