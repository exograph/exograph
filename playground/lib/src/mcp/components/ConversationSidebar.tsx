// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import React, { useState } from "react";
import { ChatConversation } from "../providers/types";
import { PROVIDERS } from "../providers/config";
import { useChatConfig } from "../context/ChatConfigContext";
import { useApiKeys } from "../context/ApiKeyContext";
import { useConversations } from "../context/ConversationContext";
import { ConfigurationModal } from "./ConfigurationModal";
import { Plus, Settings, MessageCircle, Trash2 } from "lucide-react";

interface ConversationSidebarProps {
  mcpEndpoint: string;
}

interface ConversationItemProps {
  conversation: ChatConversation;
  isActive: boolean;
  onSelect: () => void;
  onDelete: () => void;
}

function ConversationItem({
  conversation,
  isActive,
  onSelect,
  onDelete,
}: ConversationItemProps) {
  const provider = PROVIDERS[conversation.provider];

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
              {provider?.displayName || conversation.provider} •{" "}
              {conversation.messages.length} messages
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
          {(() => {
            const lastMessage =
              conversation.messages[conversation.messages.length - 1]
                ?.content || "";
            return lastMessage.length > 60
              ? lastMessage.substring(0, 60) + "..."
              : lastMessage;
          })()}
        </div>
      )}
    </div>
  );
}

export function ConversationSidebar({ mcpEndpoint }: ConversationSidebarProps) {
  const { config } = useChatConfig();
  const { hasApiKey } = useApiKeys();
  const {
    conversations,
    activeConversation,
    setActiveConversation,
    createNewConversation,
    deleteConversation,
  } = useConversations();
  const [collapsed, setCollapsed] = useState(false);
  const [showConfigModal, setShowConfigModal] = useState(false);

  // Only show non-scratch pad conversations in the sidebar
  const sortedConversations = [...conversations]
    .filter((conv) => !conv.isScratchPad)
    .sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());

  const handleNewConversation = () => {
    const newConversation = createNewConversation(
      config.provider,
      config.model
    );
    setActiveConversation(newConversation);
  };

  const handleSelectConversation = (conversation: ChatConversation) => {
    setActiveConversation(conversation);
  };

  const handleDeleteConversation = (id: string) => {
    deleteConversation(id);
  };

  if (collapsed) {
    return (
      <div
        className={`w-12 bg-white dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700`}
      >
        <div className="p-2 space-y-2">
          <button
            onClick={() => setCollapsed(false)}
            className="w-8 h-8 rounded-lg bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 flex items-center justify-center text-gray-600 dark:text-gray-400"
            title="Expand sidebar"
          >
            →
          </button>
          <button
            onClick={handleNewConversation}
            className="w-8 h-8 rounded-lg bg-blue-500 hover:bg-blue-600 text-white flex items-center justify-center"
            title="New conversation"
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`w-80 h-full bg-white dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700 flex flex-col flex-shrink-0`}
    >
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Conversations
          </h2>
          <button
            onClick={() => setCollapsed(true)}
            className="text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300"
            title="Collapse sidebar"
          >
            ←
          </button>
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
              />
            ))}
          </div>
        )}
      </div>

      <div className="p-4 border-t border-gray-200 dark:border-gray-700 space-y-3">
        <div className="text-xs text-gray-500 dark:text-gray-400">
          <div className="font-medium mb-1">MCP Endpoint</div>
          <div className="break-all">{mcpEndpoint}</div>
        </div>

        <hr className="border-gray-200 dark:border-gray-700" />

        <div className="text-xs text-gray-500 dark:text-gray-400">
          <div className="font-medium mb-1">LLM Configuration</div>
          <div>
            {config.model} (
            {PROVIDERS[config.provider]?.displayName || config.provider})
          </div>
          <div className="flex items-center gap-1 mt-1">
            <div
              className={`w-2 h-2 rounded-full ${hasApiKey(config.provider) ? "bg-green-500" : "bg-red-500"}`}
            />
            <span>
              {hasApiKey(config.provider) ? "API Key Set" : "No API Key"}
            </span>
          </div>
        </div>
        <button
          onClick={() => setShowConfigModal(true)}
          className="w-full bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 py-2 px-3 rounded-lg text-sm font-medium transition-colors duration-200 flex items-center justify-center gap-2"
        >
          <Settings className="w-4 h-4" />
          Configure
        </button>
      </div>

      <ConfigurationModal
        open={showConfigModal}
        onOpenChange={setShowConfigModal}
      />
    </div>
  );
}
