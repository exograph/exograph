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
import {
  ChatConversation,
  ChatMessage,
  LLMProvider,
  USER_ROLE,
} from "../providers/types";
import { PROVIDERS, DEFAULT_PROVIDER } from "../providers/config";

export interface ConversationContextValue {
  conversations: ChatConversation[];
  activeConversation: ChatConversation | null;
  setActiveConversation: (conversation: ChatConversation | null) => void;
  createNewConversation: (
    provider: LLMProvider,
    model: string
  ) => ChatConversation;
  updateConversation: (conversation: ChatConversation) => void;
  deleteConversation: (id: string) => void;
  addMessageToConversation: (
    conversationId: string,
    message: ChatMessage
  ) => void;
  getConversation: (id: string) => ChatConversation | undefined;
  convertScratchPadToConversation: (conversation: ChatConversation) => void;
  loading: boolean;
}

const ConversationContext = createContext<ConversationContextValue | undefined>(
  undefined
);

const STORAGE_KEYS = {
  CONVERSATIONS: "exograph-mcp-playground-conversations",
  ACTIVE_CONVERSATION: "exograph-mcp-playground-active-conversation",
} as const;

// Helper functions for localStorage persistence
const persistConversations = (conversations: ChatConversation[]): void => {
  try {
    // Only save non-scratch pad conversations
    const persistableConversations = conversations.filter(
      (conv) => !conv.isScratchPad
    );
    localStorage.setItem(
      STORAGE_KEYS.CONVERSATIONS,
      JSON.stringify(persistableConversations)
    );
  } catch (error) {
    console.error("Error saving conversations:", error);
  }
};

const persistActiveConversationId = (id: string | null): void => {
  try {
    if (id) {
      localStorage.setItem(STORAGE_KEYS.ACTIVE_CONVERSATION, id);
    } else {
      localStorage.removeItem(STORAGE_KEYS.ACTIVE_CONVERSATION);
    }
  } catch (error) {
    console.error("Error saving active conversation ID:", error);
  }
};

const loadConversationsFromStorage = (): ChatConversation[] => {
  try {
    const stored = localStorage.getItem(STORAGE_KEYS.CONVERSATIONS);
    if (!stored) return [];

    return JSON.parse(stored)
      .filter((conv: any) => !conv.isScratchPad) // Exclude scratch pads from storage
      .map((conv: any) => ({
        ...conv,
        updatedAt: new Date(conv.updatedAt),
        messages: conv.messages.map((msg: any) => ChatMessage.fromJSON(msg)),
      }));
  } catch (error) {
    console.error("Error loading conversations:", error);
    return [];
  }
};

const loadActiveConversationId = (): string | null => {
  try {
    return localStorage.getItem(STORAGE_KEYS.ACTIVE_CONVERSATION);
  } catch (error) {
    console.error("Error loading active conversation ID:", error);
    return null;
  }
};

const generateId = (): string => {
  return `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
};

export interface ConversationProviderProps {
  children: React.ReactNode;
}

export function ConversationProvider({ children }: ConversationProviderProps) {
  const [conversations, setConversations] = useState<ChatConversation[]>([]);
  const [activeConversation, setActiveConversationState] =
    useState<ChatConversation | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);

  // Load conversations from localStorage only once on mount
  useEffect(() => {
    const loadedConversations = loadConversationsFromStorage();
    const activeId = loadActiveConversationId();

    // If no conversations exist, create a scratch pad
    if (loadedConversations.length === 0) {
      const firstProvider = PROVIDERS[DEFAULT_PROVIDER];
      const scratchPadConversation: ChatConversation = {
        id: generateId(),
        title: "New Conversation",
        messages: [],
        updatedAt: new Date(),
        provider: firstProvider.id,
        model: firstProvider.defaultModel,
        isScratchPad: true,
      };

      const conversationsWithScratchPad = [scratchPadConversation];
      setConversations(conversationsWithScratchPad);
      setActiveConversationState(scratchPadConversation);

      // Don't persist scratch pad - it's temporary
      persistActiveConversationId(null); // No active conversation stored for scratch pad
    } else {
      setConversations(loadedConversations);

      if (activeId && loadedConversations.length > 0) {
        const activeConv = loadedConversations.find(
          (conv) => conv.id === activeId
        );
        setActiveConversationState(activeConv || loadedConversations[0]);
      } else {
        // Set first conversation as active if no active ID
        setActiveConversationState(loadedConversations[0]);
        persistActiveConversationId(loadedConversations[0].id);
      }
    }

    setIsLoaded(true);
  }, []);

  // Persist conversations whenever they change (but don't reload)
  useEffect(() => {
    if (isLoaded) {
      persistConversations(conversations);
    }
  }, [conversations, isLoaded]);

  const setActiveConversation = useCallback(
    (conversation: ChatConversation | null) => {
      setActiveConversationState(conversation);
      persistActiveConversationId(conversation?.id || null);
    },
    []
  );

  const createNewConversation = useCallback(
    (provider: LLMProvider, model: string): ChatConversation => {
      const newConversation: ChatConversation = {
        id: generateId(),
        title: "New Conversation",
        messages: [],
        updatedAt: new Date(),
        provider,
        model,
      };

      setConversations((prev) => [...prev, newConversation]);
      return newConversation;
    },
    []
  );

  const updateConversation = useCallback(
    (updatedConversation: ChatConversation) => {
      setConversations((prev) =>
        prev.map((conv) =>
          conv.id === updatedConversation.id
            ? { ...updatedConversation, updatedAt: new Date() }
            : conv
        )
      );

      // Update active conversation if it's the one being updated
      if (activeConversation?.id === updatedConversation.id) {
        setActiveConversationState({
          ...updatedConversation,
          updatedAt: new Date(),
        });
      }
    },
    [activeConversation]
  );

  const deleteConversation = useCallback(
    (id: string) => {
      setConversations((prev) => {
        const filtered = prev.filter((conv) => conv.id !== id);
        const nonScratchPadFiltered = filtered.filter(
          (conv) => !conv.isScratchPad
        );

        if (activeConversation?.id === id) {
          if (nonScratchPadFiltered.length === 0) {
            const firstProvider = PROVIDERS[DEFAULT_PROVIDER];
            const scratchPadConversation: ChatConversation = {
              id: generateId(),
              title: "New Conversation",
              messages: [],
              updatedAt: new Date(),
              provider: firstProvider.id,
              model: firstProvider.defaultModel,
              isScratchPad: true,
            };

            const conversationsWithScratchPad = [scratchPadConversation];
            setActiveConversationState(scratchPadConversation);
            persistActiveConversationId(null); // Don't persist scratch pad as active

            return conversationsWithScratchPad;
          } else {
            // Select the next non-scratch pad conversation
            const deletedIndex = prev
              .filter((conv) => !conv.isScratchPad)
              .findIndex((conv) => conv.id === id);
            const nextConversation =
              nonScratchPadFiltered[
                Math.min(deletedIndex, nonScratchPadFiltered.length - 1)
              ];

            setActiveConversationState(nextConversation);
            persistActiveConversationId(nextConversation.id);

            return filtered;
          }
        }

        // If we're not deleting the active conversation, just return filtered list
        return filtered;
      });
    },
    [activeConversation]
  );

  const addMessageToConversation = useCallback(
    (conversationId: string, message: ChatMessage) => {
      setConversations((prev) =>
        prev.map((conv) => {
          if (conv.id === conversationId) {
            const updatedConversation = {
              ...conv,
              messages: [...conv.messages, message],
              updatedAt: new Date(),
            };

            // Auto-generate title from first user message
            if (conv.messages.length === 0 && message.role === USER_ROLE) {
              updatedConversation.title = message.generateTitle();
            }

            // Convert scratch pad to permanent conversation on first user message
            if (
              conv.isScratchPad &&
              message.role === USER_ROLE &&
              conv.messages.length === 0
            ) {
              updatedConversation.isScratchPad = false;
              // Update active conversation ID in storage since it's now permanent
              persistActiveConversationId(conversationId);
            }

            // Update active conversation if it's the one being updated
            if (activeConversation?.id === conversationId) {
              setActiveConversationState(updatedConversation);
            }

            return updatedConversation;
          }
          return conv;
        })
      );
    },
    [activeConversation]
  );

  const getConversation = useCallback(
    (id: string): ChatConversation | undefined => {
      return conversations.find((conv) => conv.id === id);
    },
    [conversations]
  );

  const convertScratchPadToConversation = useCallback(
    (conversation: ChatConversation) => {
      if (!conversation.isScratchPad) return;

      setConversations((prev) =>
        prev.map((conv) =>
          conv.id === conversation.id ? { ...conv, isScratchPad: false } : conv
        )
      );

      if (activeConversation?.id === conversation.id) {
        const updatedConversation = { ...conversation, isScratchPad: false };
        setActiveConversationState(updatedConversation);
        persistActiveConversationId(conversation.id);
      }
    },
    [activeConversation]
  );

  const value = useMemo<ConversationContextValue>(
    () => ({
      conversations,
      activeConversation,
      setActiveConversation,
      createNewConversation,
      updateConversation,
      deleteConversation,
      addMessageToConversation,
      getConversation,
      convertScratchPadToConversation,
      loading: !isLoaded,
    }),
    [
      conversations,
      activeConversation,
      setActiveConversation,
      createNewConversation,
      updateConversation,
      deleteConversation,
      addMessageToConversation,
      getConversation,
      convertScratchPadToConversation,
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
