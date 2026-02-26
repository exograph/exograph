// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

import { useRef, useMemo, useEffect, type ReactNode } from "react";
import {
  AssistantRuntimeProvider,
  useLocalRuntime,
  useThreadRuntime,
  useThreadListItemRuntime,
} from "@assistant-ui/react";
import { createExographAdapter, type ExographAdapterDeps } from "../api/chat/ExographChatAdapter";
import { useProviderConfig } from "./ProviderConfigContext";
import { useCurrentModel } from "./ModelContext";
import { useMCPClient } from "./MCPClientContext";
import { ExographToolUI, ExographWithVarsToolUI } from "../components/chat/tools/ExographToolUI";
import { DefaultToolUI } from "../components/chat/tools/DefaultToolUI";

/**
 * Auto-generates thread titles from the first user message after a run completes.
 * Must be rendered inside AssistantRuntimeProvider.
 */
function ThreadTitleGenerator() {
  const threadRuntime = useThreadRuntime();
  const threadListItemRuntime = useThreadListItemRuntime();

  useEffect(() => {
    return threadRuntime.unstable_on("runEnd", () => {
      const { title } = threadListItemRuntime.getState();
      if (title) return; // Already has a title

      const messages = threadRuntime.getState().messages;
      const firstUserMsg = messages.find(m => m.role === "user");
      if (!firstUserMsg) return;

      const textPart = firstUserMsg.content.find(p => p.type === "text");
      if (!textPart || !("text" in textPart)) return;

      const newTitle = textPart.text.length > 60
        ? textPart.text.slice(0, 57) + "..."
        : textPart.text;

      threadListItemRuntime.rename(newTitle);
    });
  }, [threadRuntime, threadListItemRuntime]);

  return null;
}

interface ExographRuntimeProviderProps {
  children: ReactNode;
}

export function ExographRuntimeProvider({ children }: ExographRuntimeProviderProps) {
  const { getApiKey } = useProviderConfig();
  const { currentModel } = useCurrentModel();
  const mcpState = useMCPClient();

  // Use a ref for deps so the adapter reference stays stable across re-renders.
  // useLocalRuntime requires a stable adapter to avoid infinite update loops.
  const depsRef = useRef<ExographAdapterDeps>({ getApiKey, mcpState, currentModel });
  depsRef.current = { getApiKey, mcpState, currentModel };

  // Create the adapter once, reading deps from the ref at call time
  const adapter = useMemo(
    () => createExographAdapter(depsRef),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    []
  );

  const runtime = useLocalRuntime(adapter);

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ThreadTitleGenerator />
      <ExographToolUI />
      <ExographWithVarsToolUI />
      <DefaultToolUI />
      {children}
    </AssistantRuntimeProvider>
  );
}
