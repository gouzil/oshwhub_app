const CALLBACK_URL = "oshwhub://auth/callback";
const CODE_PATTERN = /^[A-Za-z0-9._~-]{1,512}$/;

export function passportLoginUrl(): string {
  const params = new URLSearchParams({
    appId: "JLC_OSHWHUB",
    redirectUrl: CALLBACK_URL,
    backCode: "1",
  });
  return `https://passport.jlc.com/login?${params.toString()}`;
}

export function parseAuthCallback(input: string): string | null {
  try {
    const url = new URL(input);
    if (
      url.protocol !== "oshwhub:" ||
      url.hostname !== "auth" ||
      url.username ||
      url.password ||
      url.port ||
      url.pathname !== "/callback" ||
      url.hash
    ) {
      return null;
    }
    const keys = [...url.searchParams.keys()];
    if (keys.length !== 1 || keys[0] !== "code") return null;
    const values = url.searchParams.getAll("code");
    return values.length === 1 && CODE_PATTERN.test(values[0]) ? values[0] : null;
  } catch {
    return null;
  }
}
