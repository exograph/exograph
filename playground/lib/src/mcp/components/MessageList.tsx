// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useEffect, useRef, memo, useState } from "react";
import { ChatMessage } from "../providers/types";

interface MessageListProps {
  messages: ChatMessage[];
  isLoading: boolean;
}

interface MessageItemProps {
  message: ChatMessage;
}

const MessageItem = memo(function MessageItem({ message }: MessageItemProps) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";

  if (isSystem) {
    return (
      <div className="flex justify-center mb-4">
        <div className="bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200 px-3 py-2 rounded-md text-sm">
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div 
      className={`flex mb-6 ${isUser ? "justify-end" : "justify-start"}`}
      role="article"
      aria-label={`${isUser ? "User" : "Assistant"} message`}
    >
      <div
        className={`flex max-w-[80%] ${isUser ? "flex-row-reverse" : "flex-row"}`}
      >
        <div
          className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium ${
            isUser
              ? "bg-blue-500 text-white ml-3"
              : "bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-200 mr-3"
          }`}
          aria-hidden="true"
        >
          {isUser ? "U" : "AI"}
        </div>
        <div
          className={`px-4 py-3 rounded-2xl ${
            isUser
              ? "bg-blue-500 text-white rounded-br-md"
              : "bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 rounded-bl-md"
          }`}
        >
          <div className="whitespace-pre-wrap break-words leading-relaxed">
            {message.content}
          </div>
          <div
            className={`text-xs mt-2 opacity-70 ${
              isUser ? "text-blue-100" : "text-gray-500 dark:text-gray-400"
            }`}
          >
            {message.timestamp.toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </div>
        </div>
      </div>
    </div>
  );
});

const LoadingIndicator = memo(function LoadingIndicator() {
  return (
    <div 
      className="flex justify-start mb-6"
      role="status"
      aria-label="AI is thinking"
    >
      <div className="flex max-w-[80%]">
        <div 
          className="flex-shrink-0 w-8 h-8 rounded-full bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-200 mr-3 flex items-center justify-center text-sm font-medium"
          aria-hidden="true"
        >
          AI
        </div>
        <div className="bg-gray-200 dark:bg-gray-700 px-4 py-3 rounded-2xl rounded-bl-md">
          <div className="flex space-x-1">
            <div className="w-2 h-2 bg-gray-400 dark:bg-gray-500 rounded-full animate-bounce [animation-delay:0ms]" />
            <div className="w-2 h-2 bg-gray-400 dark:bg-gray-500 rounded-full animate-bounce [animation-delay:100ms]" />
            <div className="w-2 h-2 bg-gray-400 dark:bg-gray-500 rounded-full animate-bounce [animation-delay:200ms]" />
          </div>
        </div>
      </div>
    </div>
  );
});

export function MessageList({ messages, isLoading }: MessageListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const [isUserScrolling, setIsUserScrolling] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll only if user hasn't manually scrolled up
  useEffect(() => {
    if (!isUserScrolling) {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages, isLoading, isUserScrolling]);

  // Track user scrolling to prevent auto-scroll interruption
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = container;
      const isNearBottom = scrollTop + clientHeight >= scrollHeight - 100;
      setIsUserScrolling(!isNearBottom);
    };

    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, []);

  if (messages.length === 0 && !isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center p-8 min-h-0">
        <div className="text-center">
          <div className="text-gray-400 dark:text-gray-500 text-lg mb-2">
            Welcome to Chat
          </div>
          <div className="text-gray-500 dark:text-gray-400 text-sm">
            Start a conversation by typing a message below
          </div>
        </div>
      </div>
    );
  }

  return (
    <div 
      ref={scrollContainerRef}
      className="flex-1 p-4 min-h-0 overflow-y-auto"
      role="log"
      aria-label="Chat messages"
      aria-live="polite"
    >
      <div className="space-y-1">
        {messages.map((message) => (
          <MessageItem key={message.id} message={message} />
        ))}
        {isLoading && <LoadingIndicator key="loading" />}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}
