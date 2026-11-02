import { Action, ActionPanel, Color, Icon, Keyboard, List } from "@raycast/api";
import { usePromise } from "@raycast/utils";
import { callTool, openApp } from "./mcp";

type Session = {
  agent: string;
  id: string;
  title: string;
  state: string;
  live: boolean;
  cwd: string;
  branch: string | null;
  repo: string;
  idle_for_seconds: number | null;
};

function since(seconds: number | null): string {
  if (seconds == null) return "";
  if (seconds < 60) return "now";
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}

function stateColor(state: string): Color {
  if (/wait|input|permission/i.test(state)) return Color.Orange;
  if (/work|running/i.test(state)) return Color.Green;
  return Color.SecondaryText;
}

export default function Agents() {
  const { data, isLoading, error, revalidate } = usePromise(() => callTool<Session[]>("list_sessions"));
  const live = (data ?? []).filter((s) => s.live);
  const ended = (data ?? []).filter((s) => !s.live).slice(0, 20);
  const row = (s: Session) => (
    <List.Item
      key={s.id}
      icon={{ source: Icon.Terminal, tintColor: stateColor(s.state) }}
      title={s.title || s.agent}
      subtitle={s.repo}
      keywords={[s.repo, s.branch ?? "", s.agent]}
      accessories={[
        ...(s.branch ? [{ tag: s.branch }] : []),
        { tag: { value: s.state, color: stateColor(s.state) } },
        { text: since(s.idle_for_seconds) },
      ]}
      actions={
        <ActionPanel>
          <Action title="Open Fubbik" icon={Icon.AppWindow} onAction={openApp} />
          <Action.ShowInFinder path={s.cwd} />
          <Action.CopyToClipboard title="Copy Session ID" content={s.id} />
          <Action
            title="Refresh"
            icon={Icon.ArrowClockwise}
            shortcut={Keyboard.Shortcut.Common.Refresh}
            onAction={revalidate}
          />
        </ActionPanel>
      }
    />
  );
  return (
    <List isLoading={isLoading} searchBarPlaceholder="Filter agents">
      {error && <List.EmptyView icon={Icon.Plug} title="Can't reach fubbik" description={error.message} />}
      <List.Section title="Live" subtitle={String(live.length)}>
        {live.map(row)}
      </List.Section>
      <List.Section title="Recently ended">{ended.map(row)}</List.Section>
    </List>
  );
}
