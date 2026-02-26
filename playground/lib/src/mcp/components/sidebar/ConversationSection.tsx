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
  ThreadListPrimitive,
  ThreadListItemPrimitive,
} from "@assistant-ui/react";
import { Plus, MessageCircle, Trash2 } from "lucide-react";

function ConversationItem() {
  return (
    <ThreadListItemPrimitive.Root
      className="group relative p-3 rounded-lg cursor-pointer transition-all duration-200 border bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700 data-[active=true]:bg-blue-50 dark:data-[active=true]:bg-blue-900/20 data-[active=true]:border-blue-200 dark:data-[active=true]:border-blue-800"
    >
      <div className="flex items-start justify-between gap-2">
        <ThreadListItemPrimitive.Trigger className="flex-1 min-w-0 flex gap-2 text-left">
          <MessageCircle className="w-4 h-4 mt-0.5 text-gray-400 dark:text-gray-500 shrink-0" />
          <div className="min-w-0">
            <div className="font-medium text-sm text-gray-900 dark:text-gray-100 truncate">
              <ThreadListItemPrimitive.Title fallback="New Conversation" />
            </div>
          </div>
        </ThreadListItemPrimitive.Trigger>
        <div className="flex flex-col items-end gap-1">
          <ThreadListItemPrimitive.Archive className="opacity-0 group-hover:opacity-100 transition-all duration-200 p-1 rounded text-gray-400 hover:text-red-500 dark:text-gray-500 dark:hover:text-red-400">
            <Trash2 className="w-3 h-3" />
          </ThreadListItemPrimitive.Archive>
        </div>
      </div>
    </ThreadListItemPrimitive.Root>
  );
}

export function ConversationSection() {
  return (
    <>
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Conversations
          </h2>
        </div>
        <ThreadListPrimitive.New className="w-full bg-blue-500 hover:bg-blue-600 text-white py-2 px-4 rounded-lg font-medium transition-colors duration-200 flex items-center justify-center gap-2">
          <Plus className="w-4 h-4" />
          New Conversation
        </ThreadListPrimitive.New>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="space-y-2">
          <ThreadListPrimitive.Items
            components={{ ThreadListItem: ConversationItem as ComponentType }}
          />
        </div>
      </div>
    </>
  );
}
