import { test, expect } from "bun:test";
import {
  filterActiveAgents,
  formatAgentTitle,
  type AgentListItem,
} from "./sessions.ts";

function agent(partial: Partial<AgentListItem> & Pick<AgentListItem, "agentId" | "name">): AgentListItem {
  return {
    summary: partial.summary ?? partial.name,
    lastModified: partial.lastModified ?? 0,
    ...partial,
  };
}

test("formatAgentTitle includes name and short id", () => {
  const title = formatAgentTitle(
    agent({
      agentId: "bc-7c05649d-7c12-45ad-8417-5db4418e7433",
      name: "Mor-273 7 phase planning",
      status: "finished",
    }),
  );
  expect(title).toContain("Mor-273 7 phase planning");
  expect(title).toContain("bc-7c05649d");
  expect(title).toContain("finished");
});

test("formatAgentTitle does not show archived label", () => {
  const title = formatAgentTitle(
    agent({
      agentId: "bc-af7dad0f-9511-4d00-89db-39a912de7019",
      name: "Mor-273 7 phase planning",
      archived: true,
    }),
  );
  expect(title).not.toContain("archived");
});

test("filterActiveAgents drops archived agents", () => {
  const items = [
    agent({
      agentId: "bc-live",
      name: "live",
      archived: false,
    }),
    agent({
      agentId: "bc-dead",
      name: "dead",
      archived: true,
    }),
    agent({
      agentId: "bc-unknown",
      name: "unknown",
    }),
  ];

  expect(filterActiveAgents(items).map((a) => a.agentId)).toEqual([
    "bc-live",
    "bc-unknown",
  ]);
});

test("filterActiveAgents returns empty when all archived", () => {
  expect(
    filterActiveAgents([
      agent({ agentId: "bc-a", name: "a", archived: true }),
    ]),
  ).toEqual([]);
});
