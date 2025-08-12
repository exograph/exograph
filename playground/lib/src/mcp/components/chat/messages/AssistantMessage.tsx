// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";
import { BaseMessage } from "../../../providers/ChatMessage";

interface AssistantMessageProps {
  message: BaseMessage;
}

interface SingleAssistantMessageProps {
  contentPart: any; // Content part from AI SDK
  timestamp: Date;
  showAvatar?: boolean;
}

const SingleAssistantMessage = memo(function SingleAssistantMessage({
  contentPart,
  timestamp,
  showAvatar = true,
}: SingleAssistantMessageProps) {
  // Extract text based on content part type
  const text = contentPart.type === "text" ? contentPart.text : "";

  // Don't render if no text content (TODO: Deal with other content types such as images)
  if (!text.trim()) {
    return null;
  }

  return (
    <div
      className="flex mb-6 justify-start"
      role="article"
      aria-label="Assistant message"
    >
      <div className="flex max-w-[80%] flex-row">
        {showAvatar && (
          <div
            className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-200 mr-3"
            aria-hidden="true"
          >
            AI
          </div>
        )}
        {!showAvatar && <div className="w-11 mr-3" />}{" "}
        {/* Spacer to align with avatar */}
        <div className="px-4 py-3 rounded-2xl bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 rounded-bl-md">
          <div className="whitespace-pre-wrap break-words leading-relaxed">
            {text}
          </div>
          <div className="text-xs mt-2 opacity-70 text-gray-500 dark:text-gray-400">
            {timestamp.toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </div>
        </div>
      </div>
    </div>
  );
});

export const AssistantMessage = memo(function AssistantMessage({
  message,
}: AssistantMessageProps) {
  const content = message.content.content;

  // Normalize content to always be an array
  const normalizedContent =
    typeof content === "string"
      ? [{ type: "text", text: content }]
      : Array.isArray(content)
        ? content
        : [];

  return (
    <>
      {normalizedContent.map((contentPart: any, index: number) => (
        <SingleAssistantMessage
          key={index}
          contentPart={contentPart}
          timestamp={message.timestamp}
          showAvatar={index === 0} // Only show avatar on first message
        />
      ))}
    </>
  );
});
