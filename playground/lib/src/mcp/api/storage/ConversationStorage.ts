// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { StorageAPI } from './StorageAPI';
import { ChatConversation } from '../types/ChatConversation';
import { deserializeMessage } from '../../providers/ChatMessage';

interface StoredConversation {
  id: string;
  title: string;
  messages: unknown[];
  updatedAt: string;
  isScratchPad?: boolean;
}

const CONVERSATIONS_KEY = 'exograph::mcp::playground::conversations';
const ACTIVE_CONVERSATION_KEY = 'exograph::mcp::playground::conversations::active';

export class ConversationStorage {
  static loadConversations(): ChatConversation[] {
    const stored = StorageAPI.getItem<StoredConversation[]>(CONVERSATIONS_KEY);
    if (!stored) return [];

    return stored
      .map((storedConversation) => new ChatConversation(
        storedConversation.isScratchPad ?? true,
        storedConversation.id,
        storedConversation.title,
        storedConversation.messages.map((msg) => deserializeMessage(msg)),
        new Date(storedConversation.updatedAt)
      ));
  }

  static saveConversations(conversations: ChatConversation[]): void {
    // Only save non-scratch pad conversations
    const persistableConversations = conversations.filter((conv) => !conv.isScratchPad);
    StorageAPI.setItem(CONVERSATIONS_KEY, persistableConversations);
  }

  static loadActiveConversationId(): string | null {
    return StorageAPI.getItem<string>(ACTIVE_CONVERSATION_KEY);
  }

  static saveActiveConversationId(id: string | null): void {
    if (id) {
      StorageAPI.setItem(ACTIVE_CONVERSATION_KEY, id);
    } else {
      StorageAPI.removeItem(ACTIVE_CONVERSATION_KEY);
    }
  }
}