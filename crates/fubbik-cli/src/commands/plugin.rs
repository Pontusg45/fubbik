use anyhow::Result;

use crate::PluginCommand;
use crate::output::{self, OutputMode};

pub fn run(command: PluginCommand, mode: OutputMode) -> Result<()> {
    let plugins = crate::plugin::discover();
    match command {
        PluginCommand::List => {
            if mode == OutputMode::Json {
                return output::json(&plugins);
            }
            if mode == OutputMode::Quiet {
                for plugin in plugins {
                    println!("{}", plugin.command);
                }
            } else if plugins.is_empty() {
                println!("No plugins found.");
            } else {
                for plugin in plugins {
                    println!("{:<24} {}", plugin.command, plugin.path.display());
                }
            }
        }
        PluginCommand::Doctor => {
            let report = serde_json::json!({
                "protocolVersion": crate::plugin::PROTOCOL_VERSION,
                "pluginCount": plugins.len(),
                "plugins": plugins,
            });
            if mode == OutputMode::Json {
                return output::json(&report);
            }
            println!("plugin protocol: {}", crate::plugin::PROTOCOL_VERSION);
            println!("plugins found:   {}", report["pluginCount"]);
            println!(
                "search path:     FUBBIK_PLUGIN_PATH, ~/.fubbik/plugins, ~/.local/share/fubbik/plugins, PATH"
            );
        }
    }
    Ok(())
}
