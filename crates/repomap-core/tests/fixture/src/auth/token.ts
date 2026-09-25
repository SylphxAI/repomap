export interface Claims { sub: string; exp: number }

export function signToken(sub: string): string {
  return encode({ sub, exp: Date.now() + 3600 });
}

export function verifyToken(token: string): Claims {
  const claims = decode(token);
  if (claims.exp < Date.now()) throw new Error("token expired");
  return claims;
}

function encode(c: Claims): string { return JSON.stringify(c); }
function decode(t: string): Claims { return JSON.parse(t); }
