export interface AssistantSetup {
  workspaceName: string;
  assistantName: string;
  color: string;
  focus: string;
  approvalRule: string;
}

export const PERSONAL_ASSISTANT_CONFIG_ID = "personal-assistant";

export function defaultAssistantSetup(labels: {
  workspaceName: string;
  assistantName: string;
  focus: string;
  approvalRule: string;
}): AssistantSetup {
  return {
    ...labels,
    color: "navy",
  };
}

export function buildAssistantInstructions(setup: AssistantSetup, missionTitle: string): string {
  return `# ${setup.assistantName}

You are my personal assistant in Houston.

## Main job
${setup.focus.trim()}

## First workflow
Set up and run: ${missionTitle}.

## Voice
- Plain, concise language. No technical jargon unless I use it first.
- Brief updates. One clear question when something important is missing.

## Skills
- Before complex or repeatable work, check whether a matching Skill exists and use it.
- When my request fits a Skill's purpose, run that Skill instead of improvising.

## Planning and approval
${setup.approvalRule.trim()}
- Before deleting data or making irreversible changes, share a short plan and wait for my yes.
- Low-risk drafting, summarizing, and local prep do not need approval.

## Work style
- Use connected apps when a task needs inbox, calendar, documents, or other accounts.
- Prefer Routines for recurring work I want on a schedule.
`;
}
