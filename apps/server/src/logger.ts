import { env } from "@fubbik/env/server";
import winston from "winston";

export const logger = winston.createLogger({
    level: env.NODE_ENV === "production" ? "info" : "debug",
    format: winston.format.combine(
        winston.format.timestamp(),
        env.NODE_ENV === "production" ? winston.format.json() : winston.format.combine(winston.format.colorize(), winston.format.simple())
    ),
    transports: [new winston.transports.Console()]
});
