export async function readExpiresAt(
  response: Pick<Response, 'json'>,
): Promise<number | null> {
  const body: unknown = await response.json().catch(() => null);
  if (
    typeof body !== 'object' ||
    body === null ||
    !('expires_at' in body) ||
    typeof body.expires_at !== 'number' ||
    !Number.isSafeInteger(body.expires_at) ||
    body.expires_at < 0
  ) {
    return null;
  }

  const date = new Date(body.expires_at * 1000);
  return Number.isFinite(date.getTime()) ? body.expires_at : null;
}
