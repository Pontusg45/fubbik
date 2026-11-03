import { Action, ActionPanel, Color, Icon, Keyboard, List } from "@raycast/api";
import { usePromise } from "@raycast/utils";
import { callTool, openApp } from "./mcp";

type Item = {
  section: "needs_you" | "in_flight" | "next";
  repo: string;
  tag: string | null;
  title: string;
  detail: string | null;
};

const SECTIONS: { key: Item["section"]; title: string; icon: { source: Icon; tintColor: Color } }[] = [
  { key: "needs_you", title: "Needs you", icon: { source: Icon.ExclamationMark, tintColor: Color.Orange } },
  { key: "in_flight", title: "In flight", icon: { source: Icon.CircleProgress50, tintColor: Color.Blue } },
  { key: "next", title: "Next up", icon: { source: Icon.Circle, tintColor: Color.SecondaryText } },
];

export default function Today() {
  const { data, isLoading, error, revalidate } = usePromise(() => callTool<Item[]>("get_today"));
  return (
    <List isLoading={isLoading} searchBarPlaceholder="Filter Today">
      {error && <List.EmptyView icon={Icon.Plug} title="Can't reach fubbik" description={error.message} />}
      {SECTIONS.map((section) => {
        const items = (data ?? []).filter((i) => i.section === section.key);
        return (
          <List.Section key={section.key} title={section.title} subtitle={String(items.length)}>
            {items.map((item, index) => (
              <List.Item
                key={`${section.key}-${index}`}
                icon={section.icon}
                title={item.title}
                subtitle={item.detail ?? undefined}
                keywords={[item.repo, item.tag ?? ""]}
                accessories={[...(item.tag ? [{ tag: item.tag }] : []), { text: item.repo }]}
                actions={
                  <ActionPanel>
                    <Action
                      title="Open in Fubbik"
                      icon={Icon.AppWindow}
                      onAction={() => openApp(item.repo ? { repo: item.repo } : { tab: "today" })}
                    />
                    <Action title="Open Today" icon={Icon.Calendar} onAction={() => openApp({ tab: "today" })} />
                    <Action.CopyToClipboard title="Copy Title" content={item.title} />
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
          </List.Section>
        );
      })}
    </List>
  );
}
