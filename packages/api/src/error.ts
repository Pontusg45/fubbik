export function dbError(set: { status: number }, message: string, err: unknown) {
  set.status = 500;
  console.error(message, err);
  return { message: "Internal server error" };
}
