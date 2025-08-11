// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { ChatConversation } from '../types/ChatConversation';
import { BaseMessage } from '../../providers/ChatMessage';
import { ConversationStorage } from '../storage/ConversationStorage';

export class ConversationManager {
  private conversations: ChatConversation[] = [];
  private activeConversationId: string | null = null;
  private listeners: Set<() => void> = new Set();

  constructor() {
    this.loadFromStorage();
    this.initialize();
  }

  private initialize(): void {
    // Set first conversation as active if no active conversation but conversations exist
    if (!this.activeConversationId && this.conversations.length > 0) {
      const firstConversation = this.conversations[0];
      this.setActiveConversation(firstConversation);
    }

    // Create scratch pad if no conversations exist
    if (this.conversations.length === 0) {
      const scratchPad = this.createNewConversation(true);
      this.setActiveConversation(scratchPad);
    }
  }

  addChangeListener(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notifyListeners(): void {
    this.listeners.forEach((listener) => listener());
  }

  private loadFromStorage(): void {
    this.conversations = ConversationStorage.loadConversations();
    this.activeConversationId = ConversationStorage.loadActiveConversationId();
  }

  private saveToStorage(): void {
    ConversationStorage.saveConversations(this.conversations);
  }

  private saveActiveConversationId(): void {
    ConversationStorage.saveActiveConversationId(this.activeConversationId);
  }

  getConversations(): ChatConversation[] {
    return [...this.conversations];
  }

  getActiveConversation(): ChatConversation | null {
    if (!this.activeConversationId) return null;
    return (
      this.conversations.find((conv) => conv.id === this.activeConversationId) ||
      null
    );
  }

  setActiveConversation(conversation: ChatConversation | null): void {
    this.activeConversationId = conversation?.id || null;
    this.saveActiveConversationId();
    this.notifyListeners();
  }

  createNewConversation(isScratchPad: boolean = false): ChatConversation {
    const newConversation = new ChatConversation(isScratchPad);
    this.conversations.push(newConversation);
    this.saveToStorage();
    this.notifyListeners();
    return newConversation;
  }

  deleteConversation(id: string): void {
    const wasActive = this.activeConversationId === id;

    // Find the next conversation before removing the current one
    let nextConversation: ChatConversation | null = null;
    if (wasActive) {
      const deletedIndex = this.conversations.findIndex((conv) => conv.id === id);

      if (deletedIndex !== -1) {
        // Remove the conversation and determine next conversation
        const remainingConversations = this.conversations.filter((conv) => conv.id !== id);
        if (remainingConversations.length > 0) {
          // Select conversation at the same index, or the last one if index is out of bounds
          const nextIndex = Math.min(deletedIndex, remainingConversations.length - 1);
          nextConversation = remainingConversations[nextIndex];
        }
      }
    }

    // Remove the conversation
    this.conversations = this.conversations.filter((conv) => conv.id !== id);

    if (wasActive) {
      if (nextConversation) {
        this.setActiveConversation(nextConversation);
      } else {
        // No conversations left, create a new scratch pad
        const scratchPad = this.createNewConversation(true);
        this.setActiveConversation(scratchPad);
      }
    }

    this.saveToStorage();
    this.notifyListeners();
  }


  addMessageToConversation(conversationId: string, message: BaseMessage): void {
    const conversationIndex = this.conversations.findIndex(
      (conv) => conv.id === conversationId
    );

    if (conversationIndex !== -1) {
      const conversation = this.conversations[conversationIndex];
      const updatedConversation = conversation.addMessage(message);

      this.conversations[conversationIndex] = updatedConversation;

      // If scratch pad was converted to permanent conversation, update active conversation ID in storage
      if (
        conversation.isScratchPad &&
        !updatedConversation.isScratchPad &&
        this.activeConversationId === conversationId
      ) {
        this.saveActiveConversationId();
      }

      this.saveToStorage();
      this.notifyListeners();
    }
  }
}