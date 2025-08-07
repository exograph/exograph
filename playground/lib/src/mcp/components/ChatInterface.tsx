// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useCallback, useState, useMemo } from 'react';
import { MessageList } from './MessageList';
import { MessageInput } from './MessageInput';
import { ChatMessage, USER_ROLE, ASSISTANT_ROLE, SYSTEM_ROLE } from '../providers/types';
import { useChatConfig } from '../context/ChatConfigContext';
import { useApiKeys } from '../context/ApiKeyContext';
import { useConversations } from '../context/ConversationContext';
import { useModel } from '../context/ModelContext';
import { generateChatResponse } from './ChatAPI';
import { PROVIDERS } from '../providers/config';

const generateId = (): string => {
  return `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
};

export function ChatInterface() {
  const { config, loading: configLoading } = useChatConfig();
  const { hasApiKey, getApiKey } = useApiKeys();
  const { activeConversation, addMessageToConversation } = useConversations();
  const { getCachedModel } = useModel();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const isConfigValid = Boolean(
    config.provider &&
    config.model &&
    (PROVIDERS[config.provider]?.requiresApiKey ? hasApiKey(config.provider) : true)
  );
  

  const handleSendMessage = useCallback(async (content: string) => {
    if (!activeConversation || !isConfigValid || isLoading || configLoading) return;

    setIsLoading(true);
    setError(null);

    const userMessage = new ChatMessage(
      generateId(),
      USER_ROLE,
      content,
      new Date()
    );

    // Add the message first - this will automatically convert scratch pad to permanent conversation
    addMessageToConversation(activeConversation.id, userMessage);

    try {
      const allMessages = [...(activeConversation.messages || []), userMessage];
      const apiKey = getApiKey(config.provider);
      const model = getCachedModel(config, apiKey);
      const assistantContent = await generateChatResponse(allMessages, model);

      const assistantMessage = new ChatMessage(
        generateId(),
        ASSISTANT_ROLE,
        assistantContent,
        new Date()
      );

      addMessageToConversation(activeConversation.id, assistantMessage);
    } catch (error) {
      console.error('Error sending message:', error);
      const errorMessage = new ChatMessage(
        generateId(),
        SYSTEM_ROLE,
        `Error: ${error instanceof Error ? error.message : 'Unknown error occurred'}`,
        new Date()
      );
      addMessageToConversation(activeConversation.id, errorMessage);
      setError(error instanceof Error ? error.message : 'Unknown error occurred');
    } finally {
      setIsLoading(false);
    }
  }, [activeConversation, config, addMessageToConversation, getApiKey, getCachedModel, isConfigValid, isLoading, configLoading]);

  // Memoize placeholder to prevent unnecessary re-renders
  const placeholder = useMemo(() => {
    if (!isConfigValid) {
      return `Configure your ${PROVIDERS[config.provider]?.displayName} API key to start...`;
    }
    if (isLoading) {
      return 'AI is thinking...';
    }
    return 'Type your message...';
  }, [isConfigValid, isLoading, config.provider]);

  if (!activeConversation) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center text-gray-500 dark:text-gray-400">
          <div className="text-lg mb-2">No conversation selected</div>
          <div className="text-sm">Create a new conversation to get started</div>
        </div>
      </div>
    );
  }

  const chatMessages: ChatMessage[] = activeConversation.messages || [];

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto min-h-0">
        <MessageList messages={chatMessages} isLoading={isLoading} />
      </div>
      
      <div className="flex-shrink-0 bg-white dark:bg-gray-800">
        <MessageInput
          onSendMessage={handleSendMessage}
          disabled={isLoading || !isConfigValid || configLoading}
          placeholder={placeholder}
        />
        {error && (
          <div className="bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 p-3 text-sm">
            Error: {error}
          </div>
        )}
      </div>
    </div>
  );
}