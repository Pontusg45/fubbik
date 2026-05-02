import winston from "winston";

const logLevel = process.env.LOG_LEVEL || (process.env.NODE_ENV === "production" ? "info" : "debug");

export const logger = winston.createLogger({
    level: logLevel,
    format: winston.format.combine(
        winston.format.timestamp(),
        process.env.NODE_ENV === "production"
            ? winston.format.json()
            : winston.format.combine(winston.format.colorize(), winston.format.simple())
    ),
    transports: [new winston.transports.Console()]
});
