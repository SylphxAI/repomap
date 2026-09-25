import { verifyToken, signToken } from "./token";

export class SessionStore {
  refresh(token: string): string {
    const claims = verifyToken(token);
    return signToken(claims.sub);
  }
}
