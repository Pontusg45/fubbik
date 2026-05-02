import { env } from "@fubbik/env/server";
import winston from "winston";

const logLevel = process.env.LOG_LEVEL || (env.NODE_ENV === "production" ? "info" : "debug");

export const logger = winston.createLogger({
    level: logLevel,
    format: winston.format.combine(
        winston.format.timestamp(),
        env.NODE_ENV === "production"
            ? winston.format.json()
            : winston.format.combine(winston.format.colorize(), winston.format.simple())
    ),
    defaultMeta: { env: env.NODE_ENV },
    transports: [
        new winston.transports.Console({
            stderrLevels: ["error"]
        })
    ]
});

export function createChildLogger(bindings: Record<string, unknown>): winston.Logger {
    return logger.child(bindings);
}
