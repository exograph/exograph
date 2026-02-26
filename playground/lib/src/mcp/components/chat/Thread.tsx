// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { type ComponentType } from "react";
import {
  ThreadPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
  useMessage,
  useThread,
} from "@assistant-ui/react";
import { MarkdownTextPrimitive } from "@assistant-ui/react-markdown";
import remarkGfm from "remark-gfm";
import { Send, ArrowDown, AlertCircle } from "lucide-react";
import { useCurrentModel } from "../../context/ModelContext";
import { useProviderConfig } from "../../context/ProviderConfigContext";
import { PROVIDERS } from "../../providers/config";

function ThreadWelcome() {
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

function UserMessage() {
  return (
    <div className="flex mb-6 justify-end">
      <div className="flex max-w-[80%] flex-row-reverse">
        <div
          className="shrink-0 w-8 h-8 rounded-full bg-blue-500 text-white ml-3 flex items-center justify-center text-sm font-medium"
          aria-hidden="true"
        >
          U
        </div>
        <div className="bg-blue-500 text-white px-4 py-3 rounded-2xl rounded-br-md">
          <MessagePrimitive.Parts
            components={{
              Text: ({ text }) => <span className="text-sm whitespace-pre-wrap">{text}</span>,
            }}
          />
        </div>
      </div>
    </div>
  );
}

const NullComponent = () => null;

function AssistantMessage() {
  const message = useMessage();
  const hasText = message.content.some(p => p.type === 'text' && p.text);
  const hasToolCalls = message.content.some(p => p.type === 'tool-call');
  const isError = message.status?.type === 'incomplete' && 'reason' in message.status && message.status.reason === 'error';
  const isTextOnlyError = isError && hasText && !hasToolCalls;

  return (
    <div className="space-y-4">
      {/* Tool calls as standalone blocks (text suppressed) */}
      <MessagePrimitive.Parts
        components={{ Text: NullComponent }}
      />
      {/* Error-only message (no tool calls): red error bubble */}
      {isTextOnlyError && (
        <div className="flex mb-6 justify-start">
          <div className="flex max-w-[80%]">
            <div
              className="shrink-0 w-8 h-8 rounded-full bg-red-500 text-white mr-3 flex items-center justify-center"
              aria-hidden="true"
            >
              <AlertCircle className="w-5 h-5" />
            </div>
            <div className="bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-200 px-4 py-3 rounded-2xl rounded-bl-md">
              <MessagePrimitive.Parts
                components={{
                  Text: ({ text }) => <span className="text-sm whitespace-pre-wrap">{text}</span>,
                  tools: { Override: NullComponent },
                }}
              />
            </div>
          </div>
        </div>
      )}
      {/* Normal text in AI bubble (tool calls suppressed) */}
      {hasText && !isTextOnlyError && (
        <div className="flex mb-6 justify-start">
          <div className="flex max-w-[80%]">
            <div
              className="shrink-0 w-8 h-8 rounded-full bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-200 mr-3 flex items-center justify-center text-sm font-medium"
              aria-hidden="true"
            >
              AI
            </div>
            <div className="bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 px-4 py-3 rounded-2xl rounded-bl-md">
              <MessagePrimitive.Parts
                components={{
                  Text: AssistantTextPart,
                  tools: { Override: NullComponent },
                }}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
function AssistantTextPart(_props: { text: string; status: unknown }) {
  return (
    <div className="prose prose-sm dark:prose-invert max-w-none">
      <MarkdownTextPrimitive remarkPlugins={[remarkGfm]} />
    </div>
  );
}

function ThinkingIndicator() {
  const thread = useThread();
  if (!thread.isRunning) return null;

  return (
    <div className="flex justify-start mb-6" role="status" aria-label="AI is thinking">
      <div className="flex max-w-[80%]">
        <div
          className="shrink-0 w-8 h-8 rounded-full bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-200 mr-3 flex items-center justify-center text-sm font-medium"
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
}

function ThreadScrollToBottom() {
  return (
    <ThreadPrimitive.ScrollToBottom asChild>
      <button className="absolute bottom-2 right-2 rounded-full bg-white dark:bg-gray-700 shadow-md p-2 hover:bg-gray-100 dark:hover:bg-gray-600 transition-colors">
        <ArrowDown className="w-4 h-4 text-gray-600 dark:text-gray-300" />
      </button>
    </ThreadPrimitive.ScrollToBottom>
  );
}

function Composer() {
  const { isConfigValid, currentModel } = useCurrentModel();
  const { getApiKey } = useProviderConfig();
  const thread = useThread();
  const isRunning = thread.isRunning;

  const placeholder = (() => {
    if (!isConfigValid) {
      const hasAnyApiKey = Object.values(PROVIDERS).some((provider) =>
        provider.requiresApiKey ? getApiKey(provider.id) !== undefined : true
      );
      if (!hasAnyApiKey) return "Set up API keys for a provider to start...";
      const currentProvider = PROVIDERS[currentModel.provider];
      return `Configure your ${currentProvider.displayName} API key to start...`;
    }
    if (isRunning) return "AI is thinking...";
    return "Type your message...";
  })();

  const isDisabled = !isConfigValid || isRunning;

  return (
    <div className="border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-4">
      <ComposerPrimitive.Root className="flex items-end gap-2">
        <div className="flex-1 relative">
          <ComposerPrimitive.Input
            autoFocus
            placeholder={placeholder}
            disabled={isDisabled}
            rows={1}
            className="w-full resize-none border border-gray-300 dark:border-gray-600 rounded-lg px-3 py-2 pr-10 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100 dark:disabled:bg-gray-800 disabled:text-gray-400 disabled:cursor-not-allowed max-h-32 overflow-y-hidden min-h-10"
          />
          {!isDisabled && (
            <div className="absolute right-2 bottom-2 text-xs text-gray-400 dark:text-gray-500">
              Enter to send
            </div>
          )}
        </div>
        <ComposerPrimitive.Send
          disabled={isDisabled}
          className="px-4 py-2 border border-transparent rounded-lg font-medium transition-colors duration-200 mb-2 flex items-center justify-center gap-2 bg-blue-500 hover:bg-blue-600 text-white active:bg-blue-700 disabled:bg-gray-300 dark:disabled:bg-gray-600 disabled:text-gray-500 dark:disabled:text-gray-400 disabled:cursor-not-allowed"
          style={{ minHeight: "2.5rem" }}
        >
          <Send className="w-4 h-4" />
          Send
        </ComposerPrimitive.Send>
      </ComposerPrimitive.Root>
    </div>
  );
}

export function Thread() {
  return (
    <ThreadPrimitive.Root className="flex flex-col h-full">
      <ThreadPrimitive.Viewport className="flex-1 overflow-y-auto min-h-0">
        <ThreadPrimitive.Empty>
          <ThreadWelcome />
        </ThreadPrimitive.Empty>
        <div className="p-4 space-y-1">
          <ThreadPrimitive.Messages
            components={{
              UserMessage: UserMessage as ComponentType,
              AssistantMessage: AssistantMessage as ComponentType,
            }}
          />
          <ThinkingIndicator />
        </div>
        <ThreadScrollToBottom />
      </ThreadPrimitive.Viewport>
      <div className="shrink-0 bg-white dark:bg-gray-800">
        <Composer />
      </div>
    </ThreadPrimitive.Root>
  );
}
