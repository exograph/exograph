// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useMemo,
} from "react";
import { BaseMessage } from "../providers/ChatMessage";
import { ConversationManager } from "../api";
import { ChatConversation } from "../api/types/ChatConversation";

export interface ConversationContextValue {
  conversations: ChatConversation[];
  activeConversation: ChatConversation | null;
  setActiveConversation: (conversation: ChatConversation | null) => void;
  createNewConversation: () => ChatConversation;
  deleteConversation: (id: string) => void;
  addMessageToConversation: (
    conversationId: string,
    message: BaseMessage
  ) => void;
  setConversationError: (conversationId: string, error: string) => void;
  getConversationError: (conversationId: string) => string | null;
  loading: boolean;
}

const ConversationContext = createContext<ConversationContextValue | undefined>(
  undefined
);

export interface ConversationProviderProps {
  children: React.ReactNode;
}

export function ConversationProvider({ children }: ConversationProviderProps) {
  const [conversationManager] = useState(() => new ConversationManager());
  const [version, setVersion] = useState(0);
  const [isLoaded, setIsLoaded] = useState(false);
  const [conversationErrors, setConversationErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    const cleanup = conversationManager.addChangeListener(() => {
      setVersion(prev => prev + 1);
    });

    setVersion(prev => prev + 1);
    setIsLoaded(true);

    return cleanup;
  }, [conversationManager]);

  const setActiveConversation = useCallback(
    (conversation: ChatConversation | null) => {
      conversationManager.setActiveConversation(conversation);
    },
    [conversationManager]
  );

  const createNewConversation = useCallback(
    (): ChatConversation => {
      return conversationManager.createNewConversation();
    },
    [conversationManager]
  );

  const deleteConversation = useCallback(
    (id: string) => {
      conversationManager.deleteConversation(id);
      // Clear any error associated with the deleted conversation
      setConversationErrors(prev => {
        const newErrors = { ...prev };
        delete newErrors[id];
        return newErrors;
      });
    },
    [conversationManager]
  );

  const addMessageToConversation = useCallback(
    (conversationId: string, message: BaseMessage) => {
      conversationManager.addMessageToConversation(conversationId, message);
      // Clear error when message is successfully added
      setConversationErrors(prev => {
        const newErrors = { ...prev };
        delete newErrors[conversationId];
        return newErrors;
      });
    },
    [conversationManager]
  );

  const setConversationError = useCallback(
    (conversationId: string, error: string) => {
      if (error) {
        setConversationErrors(prev => ({ ...prev, [conversationId]: error }));
      } else {
        setConversationErrors(prev => {
          const newErrors = { ...prev };
          delete newErrors[conversationId];
          return newErrors;
        });
      }
    },
    []
  );

  const getConversationError = useCallback(
    (conversationId: string): string | null => {
      return conversationErrors[conversationId] || null;
    },
    [conversationErrors]
  );

  const value = useMemo<ConversationContextValue>(
    () => ({
      conversations: conversationManager.getConversations(),
      activeConversation: conversationManager.getActiveConversation(),
      setActiveConversation,
      createNewConversation,
      deleteConversation,
      addMessageToConversation,
      setConversationError,
      getConversationError,
      loading: !isLoaded,
    }),
    [
      conversationManager,
      version, // Triggers re-render when manager state changes
      setActiveConversation,
      createNewConversation,
      deleteConversation,
      addMessageToConversation,
      setConversationError,
      getConversationError,
      isLoaded,
    ]
  );

  return (
    <ConversationContext.Provider value={value}>
      {children}
    </ConversationContext.Provider>
  );
}

export const useConversations = (): ConversationContextValue => {
  const context = useContext(ConversationContext);
  if (!context) {
    throw new Error(
      "useConversations must be used within a ConversationProvider"
    );
  }
  return context;
};
