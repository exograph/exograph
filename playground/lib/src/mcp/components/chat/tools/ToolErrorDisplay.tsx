// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo, type ReactNode } from "react";
import { AlertTriangle, Wifi, Clock, Shield, X } from "lucide-react";
import { isToolError, type ToolErrorType } from "../../../types/ToolError";

const iconClass = "w-5 h-5 text-red-500 shrink-0 mt-0.5";

const ERROR_INFO: Record<ToolErrorType, { icon: ReactNode; title: string; guidance: string }> = {
  network: { icon: <Wifi className={iconClass} />, title: "Network Error", guidance: "Check your internet connection and try again." },
  timeout: { icon: <Clock className={iconClass} />, title: "Request Timeout", guidance: "The request took too long. Try again or check if the service is available." },
  auth:    { icon: <Shield className={iconClass} />, title: "Authentication Error", guidance: "Please check your credentials or permissions." },
  http:    { icon: <X className={iconClass} />, title: "HTTP Error", guidance: "An HTTP error occurred. Please try again." },
  validation: { icon: <X className={iconClass} />, title: "Invalid Request", guidance: "Please check your input and try again." },
  unknown: { icon: <X className={iconClass} />, title: "Tool Error", guidance: "This error cannot be automatically resolved." },
};

interface ToolErrorDisplayProps {
  error: unknown;
  toolName?: string;
}

export const ToolErrorDisplay = memo(function ToolErrorDisplay({
  error,
  toolName,
}: ToolErrorDisplayProps) {
  const toolError = isToolError(error) ? error : null;
  const info = toolError ? (ERROR_INFO[toolError.type] ?? ERROR_INFO.unknown) : null;
  const icon = info?.icon ?? <AlertTriangle className={iconClass} />;
  const title = toolError?.type === "http" && toolError.status ? `HTTP Error ${toolError.status}` : (info?.title ?? "Tool execution failed");
  const guidance = info?.guidance ?? "";
  const errorObj = error != null && typeof error === 'object' ? error as Record<string, unknown> : null;
  const message = String(toolError?.message || errorObj?.message || errorObj?.error || error || "Unknown error occurred");

  return (
    <div className="flex items-start gap-2 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-700 rounded-lg">
      {icon}
      <div className="flex-1">
        <div className="text-sm font-medium text-red-800 dark:text-red-200">
          {title}
          {toolName && <span className="text-red-600 dark:text-red-300 font-normal"> in {toolName}</span>}
        </div>
        <div className="text-sm text-red-600 dark:text-red-300 mt-1">{message}</div>
        {guidance && <div className="text-xs text-red-500 dark:text-red-400 mt-2">{guidance}</div>}
      </div>
    </div>
  );
});
