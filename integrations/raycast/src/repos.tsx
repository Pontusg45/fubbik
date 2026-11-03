import { Action, ActionPanel, Color, Icon, Keyboard, List } from "@raycast/api";
import { usePromise } from "@raycast/utils";
import { callTool, openApp } from "./mcp";

type Repo = {
  name: string;
  path: string;
  status: string;
  missing: boolean;
  branch: string | null;
  changed_files: number | null;
  processes_up: number;
  processes: number;
  live_sessions: number;
  worktrees: number | null;
  overlaps: number;
};

function statusColor(status: string): Color {
  switch (status) {
    case "Running":
    case "External":
      return Color.Green;
    case "Degraded":
      return Color.Orange;
    case "Error":
      return Color.Red;
    default:
      return Color.SecondaryText;
  }
}

export default function Repos() {
  const { data, isLoading, error, revalidate } = usePromise(() => callTool<Repo[]>("list_repos"));
  return (
    <List isLoading={isLoading} searchBarPlaceholder="Filter repos">
      {error && <List.EmptyView icon={Icon.Plug} title="Can't reach fubbik" description={error.message} />}
      {(data ?? []).map((repo) => (
        <List.Item
          key={repo.path}
          icon={{ source: Icon.Folder, tintColor: statusColor(repo.status) }}
          title={repo.name}
          subtitle={repo.branch ?? undefined}
          accessories={[
            ...(repo.changed_files ? [{ text: `${repo.changed_files} changed` }] : []),
            ...(repo.live_sessions ? [{ tag: { value: `${repo.live_sessions} agent(s)`, color: Color.Blue } }] : []),
            ...(repo.processes ? [{ text: `${repo.processes_up}/${repo.processes} up` }] : []),
            ...(repo.overlaps ? [{ tag: { value: `${repo.overlaps} overlap(s)`, color: Color.Orange } }] : []),
            { tag: { value: repo.status, color: statusColor(repo.status) } },
          ]}
          actions={
            <ActionPanel>
              <Action title="Open in Fubbik" icon={Icon.AppWindow} onAction={() => openApp({ repo: repo.name })} />
              <Action
                title="Open Changes"
                icon={Icon.Document}
                onAction={() => openApp({ repo: repo.name, tab: "changes" })}
              />
              <Action.ShowInFinder path={repo.path} />
              <Action.OpenWith path={repo.path} />
              <Action.CopyToClipboard title="Copy Path" content={repo.path} />
              <Action
                title="Refresh"
                icon={Icon.ArrowClockwise}
                shortcut={Keyboard.Shortcut.Common.Refresh}
                onAction={revalidate}
              />
            </ActionPanel>
          }
        />
      ))}
    </List>
  );
}
