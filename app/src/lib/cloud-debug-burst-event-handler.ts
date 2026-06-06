/**
 * Pure WS event dispatch for parallel cloud burst agents (no engine-client imports).
 */

import type { HoustonEvent } from "@houston-ai/core";
import type { FeedItem } from "@houston-ai/chat";
import { queryKeys } from "./query-keys.ts";
import { queryClient } from "./query-client.ts";
import { useFeedStore } from "../stores/feeds.ts";
import { useSessionStatusStore } from "../stores/session-status.ts";

export function handleBurstHoustonEvent(event: HoustonEvent): void {
  switch (event.type) {
    case "FeedItem":
      useFeedStore.getState().pushFeedItem(
        event.data.agent_path,
        event.data.session_key,
        event.data.item as FeedItem,
        { fromWs: true },
      );
      break;
    case "SessionStatus": {
      const { status, session_key, agent_path } = event.data;
      if (
        status === "starting" ||
        status === "running" ||
        status === "completed" ||
        status === "error"
      ) {
        useSessionStatusStore.getState().setStatus(agent_path, session_key, status);
      }
      if (status === "completed" || status === "error") {
        void queryClient.invalidateQueries({ queryKey: ["activity"] });
        void queryClient.invalidateQueries({ queryKey: ["all-conversations"] });
      }
      break;
    }
    case "ActivityChanged":
      void queryClient.invalidateQueries({
        queryKey: queryKeys.activity(event.data.agent_path),
      });
      void queryClient.invalidateQueries({ queryKey: ["all-conversations"] });
      break;
    case "ConversationsChanged":
      void queryClient.invalidateQueries({
        queryKey: queryKeys.conversations(event.data.agent_path),
      });
      void queryClient.invalidateQueries({ queryKey: ["all-conversations"] });
      break;
    default:
      break;
  }
}
