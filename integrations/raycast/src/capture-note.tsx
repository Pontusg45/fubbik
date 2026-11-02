import { Action, ActionPanel, Form, Icon, popToRoot, showToast, Toast } from "@raycast/api";
import { usePromise } from "@raycast/utils";
import { callTool } from "./mcp";

type Values = { text: string; kind: string; repo: string };

export default function CaptureNote() {
  const { data: repos } = usePromise(() => callTool<{ name: string }[]>("list_repos"));

  async function submit(values: Values) {
    if (!values.text.trim()) {
      await showToast({ style: Toast.Style.Failure, title: "Write something first" });
      return;
    }
    try {
      await callTool("add_note", {
        text: values.text.trim(),
        kind: values.kind,
        ...(values.repo ? { repo: values.repo } : {}),
      });
      await showToast({ style: Toast.Style.Success, title: values.kind === "todo" ? "Todo added" : "Saved" });
      await popToRoot();
    } catch (error) {
      await showToast({ style: Toast.Style.Failure, title: "Not saved", message: String(error) });
    }
  }

  return (
    <Form
      actions={
        <ActionPanel>
          <Action.SubmitForm title="Save" icon={Icon.Check} onSubmit={submit} />
        </ActionPanel>
      }
    >
      <Form.TextArea id="text" title="Note" placeholder="What to remember" autoFocus />
      <Form.Dropdown id="kind" title="Kind" defaultValue="note">
        <Form.Dropdown.Item value="note" title="Note" />
        <Form.Dropdown.Item value="todo" title="Todo (shows on Today)" />
        <Form.Dropdown.Item value="decision" title="Decision" />
      </Form.Dropdown>
      <Form.Dropdown id="repo" title="Repo" defaultValue="">
        <Form.Dropdown.Item value="" title="None" />
        {(repos ?? []).map((r) => (
          <Form.Dropdown.Item key={r.name} value={r.name} title={r.name} />
        ))}
      </Form.Dropdown>
    </Form>
  );
}
