import { Command } from "commander";

export const doctorCommand = new Command("doctor")
    .description("Run diagnostics on the fubbik setup")
    .action(() => {
        console.log("Doctor command not yet implemented.");
    });
