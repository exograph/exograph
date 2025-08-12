// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useCallback, useState, useMemo } from "react";
import { MessageList } from "./messages/MessageList";
import { MessageInput } from "./MessageInput";
import {
  UserMessage,
  AssistantMessage,
  ToolMessage,
} from "../../providers/ChatMessage";
import { useCurrentModel } from "../../context/CurrentModelContext";
import { useConversations } from "../../context/ConversationContext";
import { useMCPClient } from "../../context/MCPClientContext";
import { useProviderConfig } from "../../context/ProviderConfigContext";
import { ModelAPI } from "../../api";
import { ChatAPI } from "../../api";
import { PROVIDERS } from "../../providers/config";
import { generateId } from "../../utils/idGenerator";

export function ChatInterface() {
  const { currentModel, isConfigValid } = useCurrentModel();
  const {
    activeConversation,
    addMessageToConversation,
    setConversationError,
    getConversationError,
  } = useConversations();
  const { hasApiKey, getApiKey } = useProviderConfig();
  const mcpState = useMCPClient();
  const isConnected = mcpState.type === "connected";
  const tools = isConnected ? mcpState.tools : null;
  const [modelAPI] = useState(() => new ModelAPI());
  const [isLoading, setIsLoading] = useState(false);

  const handleSendMessage = useCallback(
    async (content: string) => {
      if (!activeConversation || !isConfigValid || isLoading) return;

      setIsLoading(true);

      const userMessage = new UserMessage(generateId(), content, currentModel);

      // Add the message first - this will automatically convert scratch pad to permanent conversation
      addMessageToConversation(activeConversation.id, userMessage);

      try {
        const allMessages = [...activeConversation.messages, userMessage];
        const apiKey = getApiKey(currentModel.provider);
        const model = modelAPI.createModel(currentModel, apiKey, hasApiKey);
        if (!model) {
          throw new Error("Failed to create model - check your configuration");
        }
        // Only pass tools if MCP client is connected
        const response = await ChatAPI.generateChatResponse(
          allMessages,
          model,
          { tools: tools || undefined }
        );

        // If there are tool calls, add one message per tool call
        if (response.toolCalls && response.toolCalls.length > 0) {
          response.toolCalls.forEach((toolCall) => {
            const toolCallMessage = new ToolMessage(
              generateId(),
              toolCall.toolName,
              toolCall.toolCallId || generateId(), // Use toolCallId if available
              toolCall.args,
              toolCall.result,
              currentModel
            );
            addMessageToConversation(activeConversation.id, toolCallMessage);
          });
        }

        const assistantMessage = new AssistantMessage(
          generateId(),
          response.text,
          currentModel
        );

        addMessageToConversation(activeConversation.id, assistantMessage);
      } catch (error) {
        console.error("Error sending message:", error);
        const errorText =
          error instanceof Error ? error.message : "Unknown error occurred";
        setConversationError(activeConversation.id, `Error: ${errorText}`);
      } finally {
        setIsLoading(false);
      }
    },
    [
      activeConversation,
      addMessageToConversation,
      currentModel,
      getApiKey,
      getConversationError,
      hasApiKey,
      isConfigValid,
      isLoading,
      modelAPI,
      setConversationError,
      tools,
    ]
  );

  // Memoize placeholder to prevent unnecessary re-renders
  const placeholder = useMemo(() => {
    if (!isConfigValid) {
      // Check if any provider has API keys configured
      const hasAnyApiKey = Object.values(PROVIDERS).some((provider) =>
        provider.requiresApiKey ? hasApiKey(provider.id) : true
      );

      if (!hasAnyApiKey) {
        return "Set up API keys for a provider to start...";
      }

      const currentProvider = PROVIDERS[currentModel.provider];
      return `Configure your ${currentProvider.displayName} API key to start...`;
    }
    if (isLoading) {
      return "AI is thinking...";
    }
    return "Type your message...";
  }, [isConfigValid, isLoading, currentModel.provider, hasApiKey]);

  if (!activeConversation) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center text-gray-500 dark:text-gray-400">
          <div className="text-lg mb-2">No conversation selected</div>
          <div className="text-sm">
            Create a new conversation to get started
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto min-h-0">
        <MessageList
          messages={activeConversation.messages}
          isLoading={isLoading}
          error={getConversationError(activeConversation.id)}
        />
      </div>

      <div className="flex-shrink-0 bg-white dark:bg-gray-800">
        <MessageInput
          onSendMessage={handleSendMessage}
          disabled={isLoading || !isConfigValid}
          placeholder={placeholder}
        />
      </div>
    </div>
  );
}
