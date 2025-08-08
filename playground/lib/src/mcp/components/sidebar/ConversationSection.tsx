// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React from "react";
import { ChatConversation } from "../../api/types/ChatConversation";
import { useConversations } from "../../context/ConversationContext";
import { useModelSettings } from "../../context/ModelSettingsContext";
import { Plus, MessageCircle, Trash2 } from "lucide-react";

interface ConversationItemProps {
  conversation: ChatConversation;
  isActive: boolean;
  onSelect: () => void;
  onDelete: () => void;
  modelDisplayName: string;
}

function ConversationItem({
  conversation,
  isActive,
  onSelect,
  onDelete,
  modelDisplayName,
}: ConversationItemProps) {

  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete();
  };

  const formatDate = (date: Date) => {
    const hours = (Date.now() - date.getTime()) / (1000 * 60 * 60);
    return hours < 24
      ? date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      : hours < 24 * 7
        ? date.toLocaleDateString([], { weekday: "short" })
        : date.toLocaleDateString([], { month: "short", day: "numeric" });
  };

  return (
    <div
      onClick={onSelect}
      className={`group relative p-3 rounded-lg cursor-pointer transition-all duration-200 border ${
        isActive
          ? "bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800"
          : "bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700"
      }`}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0 flex gap-2">
          <MessageCircle className="w-4 h-4 mt-0.5 text-gray-400 dark:text-gray-500 flex-shrink-0" />
          <div className="min-w-0">
            <div className="font-medium text-sm text-gray-900 dark:text-gray-100 truncate">
              {conversation.title}
            </div>
            <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {modelDisplayName} • {conversation.messages.length} messages
            </div>
          </div>
        </div>
        <div className="flex flex-col items-end gap-1">
          <div className="text-xs text-gray-400 dark:text-gray-500">
            {formatDate(conversation.updatedAt)}
          </div>
          <button
            onClick={handleDeleteClick}
            className="opacity-0 group-hover:opacity-100 transition-all duration-200 p-1 rounded text-gray-400 hover:text-red-500 dark:text-gray-500 dark:hover:text-red-400"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        </div>
      </div>

      {conversation.messages.length > 0 && (
        <div className="text-xs text-gray-500 dark:text-gray-400 mt-2 truncate">
          {conversation.messages[
            conversation.messages.length - 1
          ]?.generateTitle() || ""}
        </div>
      )}
    </div>
  );
}

export function ConversationSection() {
  const {
    conversations,
    activeConversation,
    setActiveConversation,
    createNewConversation,
    deleteConversation,
  } = useConversations();
  const { availableModels } = useModelSettings();

  // Only show non-scratch pad conversations in the sidebar
  const sortedConversations = [...conversations]
    .filter((conv) => !conv.isScratchPad)
    .sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());

  const getModelDisplayName = (conversation: ChatConversation): string => {
    const modelId = conversation.currentModelId;
    if (!modelId) return 'New conversation';
    
    const model = availableModels.find(
      (m) => m.providerId === modelId.provider && m.id === modelId.model
    );
    return model?.name || `${modelId.provider}:${modelId.model}`;
  };

  const handleNewConversation = () => {
    const newConversation = createNewConversation();
    setActiveConversation(newConversation);
  };

  const handleSelectConversation = (conversation: ChatConversation) => {
    setActiveConversation(conversation);
  };

  const handleDeleteConversation = (id: string) => {
    deleteConversation(id);
  };

  return (
    <>
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Conversations
          </h2>
        </div>
        <button
          onClick={handleNewConversation}
          className="w-full bg-blue-500 hover:bg-blue-600 text-white py-2 px-4 rounded-lg font-medium transition-colors duration-200 flex items-center justify-center gap-2"
        >
          <Plus className="w-4 h-4" />
          New Conversation
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {sortedConversations.length === 0 ? (
          <div className="text-center py-8">
            <div className="text-gray-400 dark:text-gray-500 text-sm">
              No conversations yet
            </div>
            <div className="text-gray-500 dark:text-gray-400 text-xs mt-1">
              Start a new conversation to get started
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            {sortedConversations.map((conversation) => (
              <ConversationItem
                key={conversation.id}
                conversation={conversation}
                isActive={activeConversation?.id === conversation.id}
                onSelect={() => handleSelectConversation(conversation)}
                onDelete={() => handleDeleteConversation(conversation.id)}
                modelDisplayName={getModelDisplayName(conversation)}
              />
            ))}
          </div>
        )}
      </div>
    </>
  );
}
