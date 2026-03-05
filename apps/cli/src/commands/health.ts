import { Command } from "commander";

const DEFAULT_URL = "http://localhost:3000";

export const healthCommand = new Command("health")
  .description("Check API server connection")
  .option("-u, --url <url>", "server URL", DEFAULT_URL)
  .action(async (opts: { url: string }) => {
    try {
      const res = await fetch(opts.url);
      const body = await res.text();

      if (res.ok) {
        console.log(`✓ Connected to ${opts.url}`);
        console.log(`  Status: ${res.status}`);
        console.log(`  Response: ${body}`);
      } else {
        console.error(`✗ Server returned ${res.status}`);
        process.exit(1);
      }
    } catch (err) {
      console.error(`✗ Could not connect to ${opts.url}`);
      if (err instanceof Error) {
        console.error(`  ${err.message}`);
      }
      process.exit(1);
    }
  });
