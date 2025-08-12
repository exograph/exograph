// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { memo } from "react";
import { BaseMessage } from "../../../providers/ChatMessage";

interface SystemMessageProps {
  message: BaseMessage;
}

export const SystemMessage = memo(function SystemMessage({ message }: SystemMessageProps) {
  const content = message.content.content;
  const text = typeof content === 'string' ? content : '';

  return (
    <div className="flex justify-center mb-4">
      <div className="bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200 px-3 py-2 rounded-md text-sm">
        {text}
      </div>
    </div>
  );
});